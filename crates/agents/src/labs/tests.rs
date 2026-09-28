use super::*;

#[test]
fn every_mark_is_embedded_for_both_themes() {
    for one in Lab::ALL {
        if let Some(mark) = one.mark() {
            assert!(bytes(&mark.light).is_some() && bytes(&mark.dark).is_some(), "{one:?}");
        }
    }
    assert!(bytes("no/such.svg").is_none());
}
