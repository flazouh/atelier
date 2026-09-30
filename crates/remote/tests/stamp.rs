//! The built helper carries its protocol stamp, which the app reads from its bytes.
#[test]
fn the_built_helper_carries_its_stamp() {
    let bytes = std::fs::read(env!("CARGO_BIN_EXE_lathe-remote")).unwrap();
    assert_eq!(lathe_remote::ssh::speaks(&bytes), Some(lathe_remote::protocol::VERSION));
}
