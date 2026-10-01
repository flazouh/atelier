//! The built helper carries its protocol stamp, which the app reads from its bytes.
#[test]
fn the_built_helper_carries_its_stamp() {
    let bytes = std::fs::read(env!("CARGO_BIN_EXE_atelier-remote")).unwrap();
    assert_eq!(atelier_remote::ssh::speaks(&bytes), Some(atelier_remote::protocol::VERSION));
}
