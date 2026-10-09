use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, mpsc::channel},
};

use atelier_capabilities::{
    Actor, AuthKind, CapError, CapResult, Capabilities, Limits, Operation, Ref, Subscription,
    tasks::{self as v1, Change, Page, Sort, TasksProvider},
};

use super::{
    helpers::{
        account_part, activity_of, assignee_name, assignee_of, category_of, error_of,
        priority_from, ref_of, status_of, task_of, task_with_parent,
    },
    subscribe::bridge,
};
use crate::{Entry, NewTask, Patch, Query, Task, TaskId, Tracker};

const PAGE_DEFAULT: usize = 50;
const PAGE_MAX: usize = 100;

/// The tasks of one project, kept by a [`Tracker`], as a `tasks` provider.
///
/// A task's reference is `tasks:local:<account>:<key>`. The tracker keeps names, not actors, so an actor that acted through this
/// provider is remembered here for as long as the provider lives; an actor from an earlier run reads back as a person.
pub struct LocalTasks {
    tracker: Arc<dyn Tracker>,
    account: String,
    me: Actor,
    actors: Arc<Mutex<HashMap<String, Actor>>>,
}

impl LocalTasks {
    /// `account` names the project in references, and `me` is the person using the app.
    pub fn new(tracker: Arc<dyn Tracker>, account: &str, me: Actor) -> Self {
        let actors = Arc::new(Mutex::new(HashMap::from([(me.name.clone(), me.clone())])));
        Self {
            tracker,
            account: account_part(account),
            me,
            actors,
        }
    }

    fn remember(&self, actor: &Actor) {
        if let Ok(mut actors) = self.actors.lock() {
            actors.insert(actor.name.clone(), actor.clone());
        }
    }

    fn actor_named(&self, name: &str) -> Actor {
        self.actors
            .lock()
            .ok()
            .and_then(|a| a.get(name).cloned())
            .unwrap_or_else(|| Actor::person(name, name))
    }

    /// The tracker's id of a task, from its reference. The reference holds the short key.
    fn id_of(&self, task: &Ref) -> CapResult<TaskId> {
        if task.capability != "tasks" || task.provider != "local" || task.account != self.account {
            return Err(CapError::not_found(task.to_string()));
        }
        let key = task.id.to_lowercase();
        let found = self
            .tracker
            .list(&Query {
                text: Some(task.id.clone()),
                ..Query::default()
            })
            .map_err(error_of)?;
        found
            .into_iter()
            .find(|t| t.key.to_lowercase() == key)
            .map(|t| t.id)
            .ok_or_else(|| CapError::not_found(task.to_string()))
    }

    fn read(&self, task: &Ref) -> CapResult<Task> {
        let id = self.id_of(task)?;
        self.tracker
            .get(&id)
            .map_err(error_of)?
            .ok_or_else(|| CapError::not_found(task.to_string()))
    }

    fn name_of_ref(&self, r: &Ref, field: &str) -> CapResult<String> {
        if r.provider != "local" || r.account != self.account {
            return Err(CapError::invalid(field));
        }
        Ok(r.id.clone())
    }

    fn page_of<T>(all: Vec<T>, cursor: Option<&str>, limit: Option<u32>) -> CapResult<Page<T>> {
        let start = match cursor {
            Some(c) => c
                .parse::<usize>()
                .map_err(|_| CapError::invalid("cursor"))?,
            None => 0,
        };
        let size = limit.map_or(PAGE_DEFAULT, |n| (n as usize).clamp(1, PAGE_MAX));
        let end = (start + size).min(all.len());
        let next_cursor = (end < all.len()).then(|| end.to_string());
        let items = all
            .into_iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect();
        Ok(Page { items, next_cursor })
    }
}

impl TasksProvider for LocalTasks {
    fn provider(&self) -> &str {
        "local"
    }

    fn account(&self) -> &str {
        &self.account
    }

    fn capabilities(&self) -> Capabilities {
        let mut operations = Capabilities::CORE.to_vec();
        operations.push(Operation::Statuses);
        Capabilities {
            operations,
            features: vec![v1_feature_subtasks()],
            limits: Limits {
                page_max: Some(PAGE_MAX as u32),
                per_minute: None,
            },
            auth: vec![AuthKind::None],
        }
    }

    fn whoami(&self) -> CapResult<Actor> {
        Ok(self.me.clone())
    }

    fn list(&self, query: &v1::Query) -> CapResult<Page<v1::Task>> {
        let linked: Option<HashSet<TaskId>> = match &query.linked_to {
            Some(text) => {
                let r: Ref = text.parse().map_err(|_| CapError::invalid("linked_to"))?;
                if r.capability != "session" {
                    return Err(CapError::unsupported("filter by links other than sessions"));
                }
                Some(
                    self.tracker
                        .tasks_of_session(&r.id)
                        .map_err(error_of)?
                        .into_iter()
                        .collect(),
                )
            }
            None => None,
        };
        let labels: Vec<String> = query
            .labels
            .iter()
            .map(|l| self.name_of_ref(l, "labels"))
            .collect::<CapResult<_>>()?;
        let project = query
            .project
            .as_ref()
            .map(|p| self.name_of_ref(p, "project"))
            .transpose()?;
        let tracker_query = Query {
            statuses: query.status.iter().map(|c| status_of(*c)).collect(),
            assignee: query.assignee.as_deref().map(assignee_name),
            label: labels.first().cloned(),
            text: query.text.clone(),
            ..Query::default()
        };
        let everyone = self.tracker.list(&Query::default()).map_err(error_of)?;
        let keys: HashMap<TaskId, String> = everyone
            .iter()
            .map(|t| (t.id.clone(), t.key.clone()))
            .collect();
        let mut found: Vec<Task> = self.tracker.list(&tracker_query).map_err(error_of)?;
        found.retain(|t| {
            labels.iter().all(|l| t.labels.contains(l))
                && project
                    .as_ref()
                    .is_none_or(|p| t.project.as_ref() == Some(p))
                && linked.as_ref().is_none_or(|ids| ids.contains(&t.id))
        });
        match query.sort {
            Sort::Updated => {}
            Sort::Created => found.sort_by(|a, b| {
                b.created_at
                    .cmp(&a.created_at)
                    .then_with(|| b.id.cmp(&a.id))
            }),
            Sort::Priority => found.sort_by_key(|t| {
                (
                    if t.priority.number() == 0 {
                        9
                    } else {
                        t.priority.number()
                    },
                    t.key.clone(),
                )
            }),
        }
        let all = found
            .iter()
            .map(|t| {
                task_of(
                    &self.account,
                    t,
                    t.parent
                        .as_ref()
                        .and_then(|p| keys.get(p))
                        .map(String::as_str),
                )
            })
            .collect();
        Self::page_of(all, query.cursor.as_deref(), query.limit)
    }

    fn get(&self, task: &Ref) -> CapResult<v1::Task> {
        Ok(task_with_parent(
            self.tracker.as_ref(),
            &self.account,
            &self.read(task)?,
        ))
    }

    fn create(&self, new: &v1::NewTask, by: &Actor) -> CapResult<v1::Task> {
        self.remember(by);
        if new.assignees.len() > 1 {
            return Err(CapError::invalid("assignees"));
        }
        let status = match &new.status {
            Some(id) => crate::Status::parse(id).ok_or_else(|| CapError::invalid("status"))?,
            None => crate::Status::Todo,
        };
        let made = NewTask {
            title: new.title.clone(),
            description: new.description.clone(),
            status,
            priority: priority_from(new.priority),
            assignee: new.assignees.first().map(|id| assignee_of(id)),
            labels: new
                .labels
                .iter()
                .map(|l| self.name_of_ref(l, "labels"))
                .collect::<CapResult<_>>()?,
            project: new
                .project
                .as_ref()
                .map(|p| self.name_of_ref(p, "project"))
                .transpose()?,
            parent: new.parent.as_ref().map(|p| self.id_of(p)).transpose()?,
        };
        let task = self.tracker.create(&made, &by.name).map_err(error_of)?;
        Ok(task_with_parent(
            self.tracker.as_ref(),
            &self.account,
            &task,
        ))
    }

    fn update(
        &self,
        task: &Ref,
        patch: &v1::Patch,
        version: &str,
        by: &Actor,
    ) -> CapResult<v1::Task> {
        self.remember(by);
        let current = self.read(task)?;
        let now = task_with_parent(self.tracker.as_ref(), &self.account, &current);
        if now.version != version {
            return Err(CapError::Conflict {
                current: serde_json::to_value(&now).unwrap_or_default(),
            });
        }
        if patch.assignees.as_ref().is_some_and(|a| a.len() > 1) {
            return Err(CapError::invalid("assignees"));
        }
        let wanted: Option<Vec<String>> = patch
            .labels
            .as_ref()
            .map(|ls| {
                ls.iter()
                    .map(|l| self.name_of_ref(l, "labels"))
                    .collect::<CapResult<_>>()
            })
            .transpose()?;
        let (add_labels, remove_labels) = match wanted {
            Some(wanted) => (
                wanted
                    .iter()
                    .filter(|l| !current.labels.contains(l))
                    .cloned()
                    .collect(),
                current
                    .labels
                    .iter()
                    .filter(|l| !wanted.contains(l))
                    .cloned()
                    .collect(),
            ),
            None => (vec![], vec![]),
        };
        let change = Patch {
            title: patch.title.clone(),
            description: match &patch.description {
                Change::Keep => None,
                Change::Set(text) => Some(text.clone()),
                Change::Clear => Some(String::new()),
            },
            status: patch
                .status
                .as_deref()
                .map(|id| crate::Status::parse(id).ok_or_else(|| CapError::invalid("status")))
                .transpose()?,
            priority: patch.priority.map(priority_from),
            assignee: patch
                .assignees
                .as_ref()
                .map(|ids| ids.first().map(|id| assignee_of(id))),
            add_labels,
            remove_labels,
            project: match &patch.project {
                Change::Keep => None,
                Change::Set(p) => Some(Some(self.name_of_ref(p, "project")?)),
                Change::Clear => Some(None),
            },
            parent: match &patch.parent {
                Change::Keep => None,
                Change::Set(p) => Some(Some(self.id_of(p)?)),
                Change::Clear => Some(None),
            },
        };
        let id = current.id.clone();
        let task = self
            .tracker
            .update(&id, &change, &by.name)
            .map_err(error_of)?;
        Ok(task_with_parent(
            self.tracker.as_ref(),
            &self.account,
            &task,
        ))
    }

    fn comment(&self, task: &Ref, body: &str, by: &Actor) -> CapResult<v1::Comment> {
        if body.trim().is_empty() {
            return Err(CapError::invalid("body"));
        }
        self.remember(by);
        let current = self.read(task)?;
        let logged = self
            .tracker
            .record(&current.id, &Entry::Comment(body.to_string()), &by.name)
            .map_err(error_of)?;
        Ok(v1::Comment {
            reference: ref_of(&self.account, &format!("comment-{}", logged.id)),
            task: ref_of(&self.account, &current.key),
            author: by.clone(),
            body: body.to_string(),
            created_at: logged.at * 1000,
            updated_at: None,
            raw: None,
        })
    }

    fn activity(&self, task: &Ref, cursor: Option<&str>) -> CapResult<Page<v1::Activity>> {
        let current = self.read(task)?;
        let all = self
            .tracker
            .activity(&current.id)
            .map_err(error_of)?
            .iter()
            .map(|a| activity_of(&self.account, &current.key, a, self.actor_named(&a.by)))
            .collect();
        Self::page_of(all, cursor, None)
    }

    fn labels(&self) -> CapResult<Vec<v1::Label>> {
        let names = self.tracker.labels().map_err(error_of)?;
        Ok(names
            .into_iter()
            .map(|name| v1::Label {
                reference: ref_of(&self.account, &name),
                name,
                color: None,
                raw: None,
            })
            .collect())
    }

    fn projects(&self) -> CapResult<Vec<v1::Project>> {
        let mut names: Vec<String> = self
            .tracker
            .list(&Query::default())
            .map_err(error_of)?
            .into_iter()
            .filter_map(|t| t.project)
            .collect();
        names.sort();
        names.dedup();
        Ok(names
            .into_iter()
            .map(|name| v1::Project {
                reference: ref_of(&self.account, &name),
                key: name.clone(),
                name,
                raw: None,
            })
            .collect())
    }

    fn subscribe(&self) -> CapResult<Subscription<v1::Event>> {
        let (tx, rx) = channel();
        let (subscription, stop) = Subscription::new(rx);
        bridge(
            self.tracker.clone(),
            self.account.clone(),
            self.actors.clone(),
            self.tracker.subscribe(),
            tx,
            stop,
        );
        Ok(subscription)
    }

    fn statuses(&self) -> CapResult<Vec<v1::Status>> {
        Ok(crate::Status::ALL
            .iter()
            .map(|s| v1::Status::plain(category_of(*s)))
            .collect())
    }
}

fn v1_feature_subtasks() -> atelier_capabilities::Feature {
    atelier_capabilities::Feature::Subtasks
}
