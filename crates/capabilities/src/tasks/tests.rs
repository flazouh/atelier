use super::{Change, MemoryTasks, Patch, TasksProvider, contract};
use crate::Ref;

#[test]
fn the_memory_provider_passes_the_contract() {
    contract::run(&|| Box::new(MemoryTasks::new("test")));
}

#[test]
fn a_patch_reads_absent_null_and_value_apart() {
    let p: Patch = serde_json::from_str(r#"{ "title": "T" }"#).unwrap();
    assert!(p.description.is_keep() && p.project.is_keep());
    let p: Patch = serde_json::from_str(r#"{ "description": null, "project": "tasks:memory:test:P-1" }"#).unwrap();
    assert_eq!(p.description, Change::Clear);
    assert_eq!(p.project, Change::Set("tasks:memory:test:P-1".parse::<Ref>().unwrap()));
    let json = serde_json::to_string(&Patch { description: Change::Set("x".into()), ..Patch::default() }).unwrap();
    assert_eq!(json, r#"{"description":"x"}"#, "a field left alone is not written");
}

#[test]
fn a_task_reads_the_json_the_schema_shows() {
    let m = MemoryTasks::new("test");
    let t = m.create(&super::NewTask::titled("Json"), &crate::Actor::person("alex", "Alex")).unwrap();
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(v["ref"], "tasks:memory:test:MEM-1");
    assert_eq!(v["status"]["category"], "todo");
    assert_eq!(v["priority"], "none");
    assert_eq!(serde_json::from_value::<super::Task>(v).unwrap(), t);
}
