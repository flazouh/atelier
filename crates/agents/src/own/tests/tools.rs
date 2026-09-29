use std::{sync::Arc, time::{Duration, Instant}};

use lathe_project::{LocalProject, Project};
use serde_json::{Value, json};

use crate::{
    own::{
        message::Cancel,
        tools::{self, Access, Tool, ToolContext, ToolResult},
    },
    session::ToolKind,
};

struct Fixture {
    dir: tempfile::TempDir,
    project: Arc<dyn Project>,
    cancel: Cancel,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let project: Arc<dyn Project> = Arc::new(crate::testing::Locked(Arc::new(LocalProject::open(dir.path()).unwrap())));
    Fixture { dir, project, cancel: Cancel::default() }
}

fn tool(name: &str) -> &'static dyn Tool {
    tools::all().iter().find(|t| t.name() == name).map(|b| b.as_ref()).unwrap()
}

impl Fixture {
    fn run(&self, name: &str, input: Value) -> ToolResult {
        tool(name).run(&ToolContext { project: self.project.as_ref(), cancel: &self.cancel }, &input)
    }

    fn file(&self, path: &str, text: &str) {
        let full = self.dir.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, text).unwrap();
    }

    fn text(&self, path: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(path)).unwrap()
    }
}

#[test]
fn the_tools_have_kinds_access_and_schemas_the_model_can_use() {
    let table = [
        ("read", ToolKind::Read, Access::Read),
        ("list", ToolKind::Search, Access::Read),
        ("search", ToolKind::Search, Access::Read),
        ("edit", ToolKind::Edit, Access::Edit),
        ("write", ToolKind::Write, Access::Edit),
        ("shell", ToolKind::Shell, Access::Execute),
    ];
    for (name, kind, access) in table {
        let t = tool(name);
        assert_eq!((t.kind(), t.access()), (kind, access), "{name}");
        let schema = t.schema();
        assert_eq!(schema["type"], "object", "{name}");
        assert!(!t.description().is_empty());
    }
    let defs = tools::definitions(tools::all());
    assert_eq!(defs.len(), 6);
    assert_eq!(tool("edit").file(&json!({"path": "src/a.rs"})).as_deref(), Some("src/a.rs"));
    assert_eq!(tool("shell").file(&json!({"command": "ls"})), None);
}

#[test]
fn read_numbers_lines_and_pages_long_files() {
    let f = fixture();
    f.file("a.txt", "one\ntwo\nthree\nfour\nfive\n");
    assert_eq!(f.run("read", json!({"path": "a.txt"})), ToolResult::ok("1\tone\n2\ttwo\n3\tthree\n4\tfour\n5\tfive\n"));
    let page = f.run("read", json!({"path": "a.txt", "offset": 2, "limit": 2}));
    assert_eq!(page.text, "2\ttwo\n3\tthree\n[showing lines 2 to 3 of 5; use offset 4 to go on]\n");
    assert!(f.run("read", json!({"path": "a.txt", "offset": 99})).is_error);
    f.file("empty.txt", "");
    assert_eq!(f.run("read", json!({"path": "empty.txt"})), ToolResult::ok("empty.txt is empty"));
    let long: String = (1..=5000).map(|i| format!("line {i}\n")).collect();
    f.file("long.txt", &long);
    let out = f.run("read", json!({"path": "long.txt"}));
    assert!(!out.is_error && out.text.len() < 40_000 && out.text.contains("use offset"), "{}", out.text.len());
}

#[test]
fn read_refuses_binary_missing_and_outside_paths_and_accepts_an_absolute_path_inside() {
    let f = fixture();
    std::fs::write(f.dir.path().join("bin.dat"), [0u8, 1, 2, 0, 255]).unwrap();
    assert!(f.run("read", json!({"path": "bin.dat"})).text.contains("binary"));
    assert!(f.run("read", json!({"path": "nope.txt"})).text.contains("cannot read nope.txt"));
    for path in ["../secret", "a/../../secret", "/etc/passwd"] {
        let out = f.run("read", json!({"path": path}));
        assert!(out.is_error, "{path}: {out:?}");
    }
    assert!(f.run("read", json!({})).text.contains("`path` is required"));
    assert!(f.run("read", json!({"path": 5})).is_error);
    f.file("in.txt", "x\n");
    let absolute = format!("{}/in.txt", f.project.root().display());
    assert_eq!(f.run("read", json!({"path": absolute})).text, "1\tx\n");
    assert_eq!(f.run("read", json!({"path": "./in.txt"})).text, "1\tx\n");
}

#[test]
fn write_makes_folders_and_replaces_whole_files_and_stays_inside_the_project() {
    let f = fixture();
    let out = f.run("write", json!({"path": "deep/er/new.txt", "content": "héllo"}));
    assert_eq!(out, ToolResult::ok("Wrote 6 bytes to deep/er/new.txt"));
    assert_eq!(f.text("deep/er/new.txt"), "héllo");
    f.run("write", json!({"path": "deep/er/new.txt", "content": "second"}));
    assert_eq!(f.text("deep/er/new.txt"), "second");
    assert!(f.run("write", json!({"path": "../escape.txt", "content": "x"})).is_error);
    assert!(!f.dir.path().parent().unwrap().join("escape.txt").exists());
    assert!(f.run("write", json!({"path": "a.txt"})).is_error, "content is required");
    assert!(f.run("write", json!({"path": "a/../../b.txt", "content": "x"})).is_error);
}

#[test]
fn edit_replaces_exactly_once_and_says_why_it_cannot() {
    let f = fixture();
    f.file("a.rs", "fn one() {}\nfn two() {}\nfn two_again() {}\n");
    assert!(f.run("edit", json!({"path": "a.rs", "old_string": "fn one() {}", "new_string": "fn uno() {}"})).text.contains("1 replacement"));
    assert_eq!(f.text("a.rs"), "fn uno() {}\nfn two() {}\nfn two_again() {}\n");
    let many = f.run("edit", json!({"path": "a.rs", "old_string": "fn two", "new_string": "fn dos"}));
    assert!(many.is_error && many.text.contains("appears 2 times") && many.text.contains("replace_all"), "{many:?}");
    assert!(f.text("a.rs").contains("fn two()"), "nothing changed");
    let all = f.run("edit", json!({"path": "a.rs", "old_string": "fn two", "new_string": "fn dos", "replace_all": true}));
    assert!(all.text.contains("2 replacements"));
    assert_eq!(f.text("a.rs"), "fn uno() {}\nfn dos() {}\nfn dos_again() {}\n");
    let missing = f.run("edit", json!({"path": "a.rs", "old_string": "fn nothing", "new_string": "x"}));
    assert!(missing.is_error && missing.text.contains("was not found"), "{missing:?}");
    assert!(f.run("edit", json!({"path": "a.rs", "old_string": "", "new_string": "x"})).text.contains("write"));
    assert!(f.run("edit", json!({"path": "a.rs", "old_string": "fn uno", "new_string": "fn uno"})).text.contains("same"));
    assert!(f.run("edit", json!({"path": "gone.rs", "old_string": "a", "new_string": "b"})).is_error);
    assert!(f.run("edit", json!({"path": "a.rs", "old_string": "a"})).is_error);
}

#[test]
fn edit_keeps_line_ends_and_whitespace_exactly() {
    let f = fixture();
    f.file("w.txt", "a\r\n  b\t\r\nc");
    f.run("edit", json!({"path": "w.txt", "old_string": "  b\t\r\n", "new_string": "B\r\n"}));
    assert_eq!(f.text("w.txt"), "a\r\nB\r\nc");
    std::fs::write(f.dir.path().join("bin"), [0xffu8, 0xfe]).unwrap();
    assert!(f.run("edit", json!({"path": "bin", "old_string": "a", "new_string": "b"})).text.contains("UTF-8"));
}

#[test]
fn list_shows_a_folder_marks_folders_and_says_when_nothing_is_there() {
    let f = fixture();
    f.file("src/a.rs", "");
    f.file("src/deep/b.rs", "");
    f.file("top.txt", "");
    let all = f.run("list", json!({}));
    assert!(all.text.lines().any(|l| l == "src/") && all.text.lines().any(|l| l == "src/deep/b.rs") && all.text.lines().any(|l| l == "top.txt"), "{}", all.text);
    let src = f.run("list", json!({"path": "src"}));
    assert!(src.text.contains("src/a.rs") && !src.text.contains("top.txt"), "{}", src.text);
    assert!(f.run("list", json!({"path": "nothing"})).is_error);
    assert!(f.run("list", json!({"path": "../"})).is_error);
    assert!(fixture().run("list", json!({})).is_error, "an empty project");
}

#[test]
fn list_caps_a_huge_tree() {
    let f = fixture();
    for i in 0..600 {
        f.file(&format!("d/f{i:04}.txt"), "");
    }
    let out = f.run("list", json!({"path": "d"}));
    assert_eq!(out.text.lines().filter(|l| l.starts_with("d/")).count(), 500);
    assert!(out.text.contains("500 of 600 entries shown"));
}

#[test]
fn search_finds_lines_with_numbers_and_honors_case_and_regex_and_limit() {
    let f = fixture();
    f.file("a.txt", "Alpha\nbeta\nALPHA beta\n");
    f.file("b.txt", "gamma beta\n");
    let hits = f.run("search", json!({"pattern": "alpha"}));
    assert_eq!(hits.text.lines().collect::<Vec<_>>(), ["a.txt:1: Alpha", "a.txt:3: ALPHA beta"]);
    assert_eq!(f.run("search", json!({"pattern": "alpha", "case_sensitive": true})).text, "No matches");
    assert!(f.run("search", json!({"pattern": "^b\\w+$", "regex": true})).text.contains("a.txt:2: beta"));
    let limited = f.run("search", json!({"pattern": "beta", "limit": 1}));
    assert!(limited.text.contains("[stopped at 1 lines"), "{}", limited.text);
    assert!(f.run("search", json!({"pattern": ""})).is_error);
    assert!(f.run("search", json!({})).is_error);
}

#[test]
fn shell_returns_output_and_exit_code_with_stderr_in_order() {
    let f = fixture();
    let ok = f.run("shell", json!({"command": "echo out; echo err 1>&2; echo out2"}));
    assert_eq!(ok, ToolResult::ok("out\nerr\nout2\n\n[exit code 0]"));
    let quiet = f.run("shell", json!({"command": "true"}));
    assert_eq!(quiet, ToolResult::ok("(no output; exit code 0)"));
    let fail = f.run("shell", json!({"command": "echo nope; exit 3"}));
    assert!(fail.is_error && fail.text.ends_with("[exit code 3]") && fail.text.contains("nope"), "{fail:?}");
    let cwd = f.run("shell", json!({"command": "pwd"}));
    assert!(cwd.text.starts_with(&f.project.root().display().to_string()), "runs in the project folder: {}", cwd.text);
    assert!(f.run("shell", json!({"command": "  "})).is_error);
    f.file("data.txt", "written\n");
    assert!(f.run("shell", json!({"command": "cat data.txt"})).text.starts_with("written"));
}

#[test]
fn shell_stops_a_command_that_runs_too_long() {
    let f = fixture();
    let started = Instant::now();
    let out = f.run("shell", json!({"command": "echo before; sleep 30", "timeout_secs": 1}));
    assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    assert!(out.is_error && out.text.contains("timed out") && out.text.contains("before"), "{out:?}");
}

#[test]
fn shell_stops_when_cancelled() {
    let f = fixture();
    let cancel = f.cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        cancel.set();
    });
    let started = Instant::now();
    let out = f.run("shell", json!({"command": "sleep 30"}));
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(out.is_error && out.text.contains("stopped by the user"), "{out:?}");
}

#[test]
fn shell_output_is_capped_and_a_command_that_prints_a_lot_does_not_block() {
    let f = fixture();
    let started = Instant::now();
    let out = f.run("shell", json!({"command": "head -c 3000000 /dev/zero | tr '\\0' 'x'; echo"}));
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(out.text.len() < 31_000, "{}", out.text.len());
    assert!(out.text.contains("output cut"), "the cut is announced");
}

#[test]
fn cap_cuts_on_a_character_edge_and_says_how_much_is_missing() {
    let text = "é".repeat(100);
    let capped = tools::cap(&text, 11);
    assert!(capped.starts_with(&"é".repeat(5)) && capped.contains("output cut at 11 bytes"), "{capped}");
    assert_eq!(tools::cap("short", 100), "short");
}
