use super::first_error_line;

#[test]
fn a_compiler_error_wins_over_the_exit_line() {
    let log = "2026-09-29T04:00:04.0000001Z running 3 tests
2026-09-29T04:00:05.0000001Z error[E0308]: mismatched types
2026-09-29T04:00:06.0000001Z ##[error]Process completed with exit code 101.";
    assert_eq!(first_error_line(log).as_deref(), Some("error[E0308]: mismatched types"));
}

#[test]
fn an_annotation_says_it_first() {
    let log = "2026-09-29T04:00:05.0000001Z error: something earlier
2026-09-29T04:00:06.0000001Z ##[error]src/a.ts(3,1): Type 'x' is not assignable
2026-09-29T04:00:06.0000002Z ##[error]Process completed with exit code 2.";
    assert_eq!(first_error_line(log).as_deref(), Some("src/a.ts(3,1): Type 'x' is not assignable"));
}

#[test]
fn a_panic_and_a_timeout_read_as_errors() {
    assert_eq!(
        first_error_line("ok\nthread 'main' panicked at src/lib.rs:3:5:\n").as_deref(),
        Some("thread 'main' panicked at src/lib.rs:3:5:")
    );
    assert_eq!(first_error_line("Timed out after 50ms waiting for the socket").as_deref(), Some("Timed out after 50ms waiting for the socket"));
}

#[test]
fn colours_go() {
    assert_eq!(first_error_line("\u{1b}[1;31merror:\u{1b}[0m the key is missing").as_deref(), Some("error: the key is missing"));
}

#[test]
fn the_exit_line_is_the_last_resort() {
    let log = "all good\n##[error]Process completed with exit code 1.";
    assert_eq!(first_error_line(log).as_deref(), Some("Process completed with exit code 1."));
    assert_eq!(first_error_line("all good\nbye"), None);
}

#[test]
fn a_long_line_is_cut() {
    let line = first_error_line(&format!("error: {}", "x".repeat(400))).unwrap();
    assert_eq!(line.chars().count(), 200);
    assert!(line.ends_with('…'));
}
