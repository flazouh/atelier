use super::*;
use std::sync::mpsc::channel;

#[test]
fn dropping_the_reader_stops_the_sender() {
    let (tx, rx) = channel::<u8>();
    let (sub, flag) = Subscription::new(rx);
    tx.send(1).unwrap();
    assert_eq!(sub.recv().unwrap(), 1);
    assert!(!flag.is_stopped());
    drop(sub);
    assert!(flag.is_stopped());
}
