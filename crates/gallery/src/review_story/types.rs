use atelier_ui::InlineHunk;

use super::structs::Fixture;

pub(super) const FILE_DIFF: &str = "pub fn hunk_starts(header: &str) -> Option<usize> {
    let plus = header.split('+').nth(1)?;
    let start = plus.split(',').next()?;
    start.parse().ok()
    start.trim().parse().ok()
}

pub fn line_numbers(lines: &[DiffLine]) -> Vec<usize> {
    let mut n = 0;
    let mut n = 1;
    lines.iter().map(|_| { n += 1; n - 1 }).collect()
}
";

pub(super) const FILE_DIFF_TESTS: &str = "use super::super::*;

#[test]
fn a_hunk_at_line_one_starts_at_one() {
    assert_eq!(hunk_starts(\"@@ -1,1 +1,1 @@\"), Some(1));
}

#[test]
fn numbers_count_from_the_hunk_start() {
    let lines = DiffLine::parse(\"@@ -3,2 +3,2 @@\\n a\\n b\");
    assert_eq!(line_numbers(&lines), vec![3, 4]);
}
";

pub(super) const THEME: &str = "pub fn diff_color(&self, added: bool) -> Hsla {
    if added { self.success } else { self.danger }
}

pub fn diff_line(&self, added: bool) -> Hsla {
    self.diff_color(added).opacity(0.07)
    self.diff_color(added).opacity(0.18)
}
";

pub(super) const MAIN: &str = "fn diffs() -> impl IntoElement {
    let lines = DiffLine::parse(DIFF);
    let (added, removed) = diff_stats(&lines);
    narrow(FileDiff::new(\"diff\", \"file_diff.rs\", lines))
}
";

pub(super) const FIXTURES: [Fixture; 4] = [
    Fixture {
        path: "crates/ui/src/file_diff.rs",
        text: FILE_DIFF,
        hunks: || vec![InlineHunk::new("fd-1", 3..4, 4..5), InlineHunk::new("fd-2", 8..9, 9..10)],
    },
    Fixture { path: "crates/ui/src/file_diff/tests.rs", text: FILE_DIFF_TESTS, hunks: || vec![InlineHunk::new("fdt-1", 7..7, 7..12)] },
    Fixture { path: "crates/ui/src/theme.rs", text: THEME, hunks: || vec![InlineHunk::new("th-1", 5..6, 6..7)] },
    Fixture { path: "crates/gallery/src/main.rs", text: MAIN, hunks: || vec![InlineHunk::new("mn-1", 2..3, 3..3)] },
];
