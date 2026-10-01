use super::*;
/// Each signal says its name and what usually sends it.
#[test]
fn each_signal_says_why_the_app_ends() {
    assert_eq!(words(libc::SIGTERM), b"exit on SIGTERM: something asked atelier to stop\n");
    assert_eq!(words(libc::SIGINT), b"exit on SIGINT: Ctrl+C where atelier was started\n");
    assert_eq!(words(libc::SIGHUP), b"exit on SIGHUP: the terminal or the session that started atelier went\n");
    assert_eq!(words(libc::SIGQUIT), b"exit on SIGQUIT\n");
}
