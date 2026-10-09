//! Stand-ins for `gh`. [`Fake`] behaves like a small GitHub that holds one repository, `acme/widgets`. [`Script`]
//! replays the answers a test lists, in order, and records what was asked.
use std::{
    collections::{HashMap, VecDeque},
    hash::{DefaultHasher, Hash, Hasher},
    sync::{Arc, Mutex},
};

use serde_json::{Value, json};

use crate::{
    runner::{Call, Failure, Gh, Method, Reply},
    time,
};

pub fn reply(status: u16, headers: &[(&str, &str)], body: &Value) -> Reply {
    Reply {
        status,
        headers: headers
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect(),
        body: if body.is_null() {
            String::new()
        } else {
            body.to_string()
        },
    }
}

pub fn fixture(name: &str) -> Value {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")))
        .unwrap()
}

/// Replays answers in order and fails the test when the provider asks for more or for something else.
pub struct Script {
    steps: Mutex<VecDeque<(String, Result<Reply, Failure>)>>,
    seen: Mutex<Vec<Call>>,
}

impl Script {
    /// Each step is `"GET /path?query"` and the answer to give.
    pub fn new(steps: Vec<(&str, Result<Reply, Failure>)>) -> Arc<Self> {
        Arc::new(Self {
            steps: Mutex::new(steps.into_iter().map(|(k, r)| (k.to_string(), r)).collect()),
            seen: Mutex::default(),
        })
    }

    pub fn seen(&self) -> Vec<Call> {
        self.seen.lock().unwrap().clone()
    }
}

impl Gh for Script {
    fn send(&self, call: &Call) -> Result<Reply, Failure> {
        let (expected, answer) = self.steps.lock().unwrap().pop_front().unwrap_or_else(|| {
            panic!("an unexpected call: {} {}", call.method.as_str(), call.path)
        });
        assert_eq!(format!("{} {}", call.method.as_str(), call.path), expected);
        self.seen.lock().unwrap().push(call.clone());
        answer
    }
}

struct State {
    issues: Vec<Value>,
    comments: HashMap<u64, Vec<Value>>,
    next_number: u64,
    next_comment: u64,
    /// Seconds since the epoch. Every change moves it on by one, so each change has its own `updated_at`.
    clock: i64,
    /// Answers to give instead of the normal ones, one per call.
    overrides: VecDeque<Result<Reply, Failure>>,
    calls: Vec<String>,
}

pub struct Fake {
    state: Mutex<State>,
}

impl Fake {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                issues: Vec::new(),
                comments: HashMap::new(),
                next_number: 1,
                next_comment: 1,
                clock: time::parse("2026-01-01T00:00:00Z").unwrap() / 1000,
                overrides: VecDeque::new(),
                calls: Vec::new(),
            }),
        })
    }

    /// The next call gets this answer, whatever it is.
    pub fn answer_next(&self, answer: Result<Reply, Failure>) {
        self.state.lock().unwrap().overrides.push_back(answer);
    }

    /// `"METHOD path"` of each call, with `[etag]` after one that sent `If-None-Match`.
    pub fn calls(&self) -> Vec<String> {
        self.state.lock().unwrap().calls.clone()
    }

    /// An issue that somebody else opened.
    pub fn external_issue(&self, title: &str) -> u64 {
        let mut s = self.state.lock().unwrap();
        make_issue(&mut s, json!({ "title": title }), false)["number"]
            .as_u64()
            .unwrap()
    }

    pub fn external_pull_request(&self, title: &str) -> u64 {
        let mut s = self.state.lock().unwrap();
        make_issue(&mut s, json!({ "title": title }), true)["number"]
            .as_u64()
            .unwrap()
    }

    pub fn external_title(&self, number: u64, title: &str) {
        let mut s = self.state.lock().unwrap();
        let at = tick(&mut s);
        let issue = s.issues.iter_mut().find(|i| i["number"] == number).unwrap();
        issue["title"] = json!(title);
        issue["updated_at"] = json!(at);
    }
}

fn tick(s: &mut State) -> String {
    s.clock += 1;
    time::format(s.clock * 1000)
}

fn user() -> Value {
    json!({ "login": "tester" })
}

fn labels_json(names: &[Value]) -> Value {
    names
        .iter()
        .map(|n| json!({ "name": n, "color": "ededed" }))
        .collect()
}

fn make_issue(s: &mut State, body: Value, pull_request: bool) -> Value {
    let at = tick(s);
    let number = s.next_number;
    s.next_number += 1;
    let names = body["labels"].as_array().cloned().unwrap_or_default();
    let logins = body["assignees"].as_array().cloned().unwrap_or_default();
    let mut issue = json!({
        "number": number,
        "title": body["title"],
        "body": body["body"],
        "state": "open",
        "state_reason": null,
        "user": user(),
        "labels": labels_json(&names),
        "assignees": logins.iter().map(|l| json!({ "login": l })).collect::<Vec<_>>(),
        "comments": 0,
        "created_at": at,
        "updated_at": at,
    });
    if pull_request {
        issue["pull_request"] = json!({});
    }
    s.issues.push(issue.clone());
    issue
}

fn decode(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let hex = text
            .get(i + 1..i + 3)
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (b[i], hex) {
            (b'%', Some(v)) => {
                out.push(v);
                i += 3;
            }
            _ => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn params(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.to_string(), decode(v)))
        .collect()
}

fn not_found() -> Result<Reply, Failure> {
    Ok(reply(404, &[], &json!({ "message": "Not Found" })))
}

impl Gh for Fake {
    fn send(&self, call: &Call) -> Result<Reply, Failure> {
        let mut s = self.state.lock().unwrap();
        s.calls.push(format!(
            "{} {}{}",
            call.method.as_str(),
            call.path,
            if call.if_none_match.is_some() {
                " [etag]"
            } else {
                ""
            }
        ));
        if let Some(answer) = s.overrides.pop_front() {
            return answer;
        }
        let (path, query) = call.path.split_once('?').unwrap_or((&call.path, ""));
        let q = params(query);
        let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
        let body = call.body.clone().unwrap_or_default();
        match (call.method, segments.as_slice()) {
            (Method::Get, ["user"]) => Ok(reply(
                200,
                &[],
                &json!({ "login": "tester", "name": "Tess Ter" }),
            )),
            (m, ["repos", "acme", "widgets", rest @ ..]) => {
                repo(&mut s, m, rest, &q, &body, call, path)
            }
            _ => not_found(),
        }
    }
}

fn repo(
    s: &mut State,
    method: Method,
    rest: &[&str],
    q: &HashMap<String, String>,
    body: &Value,
    call: &Call,
    path: &str,
) -> Result<Reply, Failure> {
    let number = |n: &str| n.parse::<u64>().ok();
    match (method, rest) {
        (Method::Get, ["issues"]) => list(s, q, call, path),
        (Method::Post, ["issues"]) => {
            if body["title"].as_str().unwrap_or_default().is_empty() {
                return Ok(reply(
                    422,
                    &[],
                    &json!({ "message": "Validation Failed", "errors": [{ "field": "title" }] }),
                ));
            }
            Ok(reply(201, &[], &make_issue(s, body.clone(), false)))
        }
        (Method::Get, ["issues", n]) => {
            match number(n).and_then(|n| s.issues.iter().find(|i| i["number"] == n)) {
                Some(issue) => Ok(reply(200, &[], issue)),
                None => not_found(),
            }
        }
        (Method::Patch, ["issues", n]) => {
            let at = tick(s);
            let Some(issue) =
                number(n).and_then(|n| s.issues.iter_mut().find(|i| i["number"] == n))
            else {
                return not_found();
            };
            for key in ["title", "body", "state", "state_reason"] {
                if body.get(key).is_some() {
                    issue[key] = body[key].clone();
                }
            }
            if issue["state"] == "open" {
                // GitHub drops the reason of an issue that is open again.
                issue["state_reason"] = Value::Null;
            }
            if let Some(names) = body["labels"].as_array() {
                issue["labels"] = labels_json(names);
            }
            if let Some(logins) = body["assignees"].as_array() {
                issue["assignees"] = logins.iter().map(|l| json!({ "login": l })).collect();
            }
            issue["updated_at"] = json!(at);
            Ok(reply(200, &[], issue))
        }
        (Method::Post, ["issues", n, "comments"]) => {
            let at = tick(s);
            let Some(n) = number(n).filter(|n| s.issues.iter().any(|i| i["number"] == *n)) else {
                return not_found();
            };
            let id = s.next_comment;
            s.next_comment += 1;
            let comment = json!({ "id": id, "user": user(), "body": body["body"], "created_at": at, "updated_at": at });
            s.comments.entry(n).or_default().push(comment.clone());
            let issue = s.issues.iter_mut().find(|i| i["number"] == n).unwrap();
            issue["updated_at"] = json!(at);
            Ok(reply(201, &[], &comment))
        }
        (Method::Get, ["issues", n, "timeline"]) => {
            let Some(n) = number(n).filter(|n| s.issues.iter().any(|i| i["number"] == *n)) else {
                return not_found();
            };
            let mut events: Vec<Value> = s
                .comments
                .get(&n)
                .into_iter()
                .flatten()
                .map(|c| json!({ "event": "commented", "id": c["id"], "user": c["user"], "body": c["body"], "created_at": c["created_at"] }))
                .collect();
            let issue = s.issues.iter().find(|i| i["number"] == n).unwrap();
            if issue["state"] == "closed" {
                events.push(json!({ "event": "closed", "id": 9000 + n, "actor": user(), "state_reason": issue["state_reason"], "created_at": issue["updated_at"] }));
            }
            Ok(reply(200, &[], &Value::Array(events)))
        }
        (Method::Get, ["labels"]) => {
            let mut names: Vec<String> = s
                .issues
                .iter()
                .flat_map(|i| i["labels"].as_array().cloned().unwrap_or_default())
                .filter_map(|l| l["name"].as_str().map(str::to_string))
                .collect();
            names.sort();
            names.dedup();
            let names: Vec<Value> = names.into_iter().map(Value::from).collect();
            Ok(reply(200, &[], &labels_json(&names)))
        }
        _ => not_found(),
    }
}

fn list(s: &State, q: &HashMap<String, String>, call: &Call, path: &str) -> Result<Reply, Failure> {
    let state = q.get("state").map_or("open", String::as_str);
    let wanted: Vec<&str> = q
        .get("labels")
        .map_or_else(Vec::new, |l| l.split(',').collect());
    let mut rows: Vec<&Value> = s
        .issues
        .iter()
        .filter(|i| state == "all" || i["state"] == state)
        .filter(|i| {
            wanted.iter().all(|w| {
                i["labels"]
                    .as_array()
                    .is_some_and(|ls| ls.iter().any(|l| l["name"] == *w))
            })
        })
        .filter(|i| {
            q.get("assignee").is_none_or(|a| {
                i["assignees"]
                    .as_array()
                    .is_some_and(|xs| xs.iter().any(|x| x["login"] == *a))
            })
        })
        .collect();
    let by_created = q.get("sort").is_some_and(|s| s == "created");
    rows.sort_by_key(|i| {
        let key = if by_created {
            i["created_at"].as_str()
        } else {
            i["updated_at"].as_str()
        };
        (key.unwrap_or_default().to_string(), i["number"].as_u64())
    });
    rows.reverse();
    let per_page: usize = q.get("per_page").and_then(|p| p.parse().ok()).unwrap_or(30);
    let page: usize = q.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
    let more = rows.len() > page * per_page;
    let slice: Vec<Value> = rows
        .into_iter()
        .skip((page - 1) * per_page)
        .take(per_page)
        .cloned()
        .collect();
    let body = Value::Array(slice);
    let mut hasher = DefaultHasher::new();
    body.to_string().hash(&mut hasher);
    let etag = format!("W/\"{:x}\"", hasher.finish());
    if call.if_none_match.as_deref() == Some(etag.as_str()) {
        return Ok(reply(304, &[("etag", &etag)], &Value::Null));
    }
    let base: String = call
        .path
        .split_once('?')
        .map_or("", |(_, query)| query)
        .split('&')
        .filter(|p| !p.starts_with("page="))
        .collect::<Vec<_>>()
        .join("&");
    let link = format!(
        "<https://api.github.com{path}?{base}&page={}>; rel=\"next\"",
        page + 1
    );
    let mut headers = vec![("etag", etag.as_str())];
    if more {
        headers.push(("link", link.as_str()));
    }
    Ok(reply(200, &headers, &body))
}
