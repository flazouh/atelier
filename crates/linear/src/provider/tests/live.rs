use atelier_capabilities::tasks::{Query, TasksProvider};

use crate::LinearTasks;

/// Reads only: whoami, a list of five, and the first of them. Run by hand with a key in the environment:
/// `LINEAR_API_KEY=... cargo test -p atelier-linear -- --ignored live`
#[test]
#[ignore = "needs LINEAR_API_KEY and the network"]
fn live_a_workspace_reads() {
    let key = std::env::var("LINEAR_API_KEY").expect("LINEAR_API_KEY");
    let linear = LinearTasks::connect(&key).expect("connect");
    let me = linear.whoami().expect("whoami");
    assert!(!me.id.is_empty() && !me.name.is_empty());
    let page = linear
        .list(&Query {
            limit: Some(5),
            ..Query::default()
        })
        .expect("list");
    assert!(page.items.len() <= 5);
    let first = page.items.first().expect("a workspace with a task");
    assert_eq!(first.reference.provider, "linear");
    assert_eq!(first.reference.account, linear.account());
    let again = linear.get(&first.reference).expect("get");
    assert_eq!(again.key, first.key);
    let states = linear.statuses().expect("statuses");
    assert!(!states.is_empty());
    println!(
        "live: {} tasks of {}, first {} ({:?}), {} statuses",
        page.items.len(),
        linear.account(),
        again.key,
        again.status.category,
        states.len()
    );
}
