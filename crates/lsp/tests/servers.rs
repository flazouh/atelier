//! Talks to real language servers over stdio: the only proof that the worker works for a language.
//! Every language runs the same checks on the same small program, written in that language:
//!
//! - From a call, go to definition lands on the declaration.
//! - On the declaration, it falls back to the uses and lands on the call.
//! - The type error in the program is reported on its line.
//! - Hover on the call names the function.
//! - After two lines are added on top, the answers move down with the text.
//!
//! The call line holds an emoji before the call, so a server that counts UTF-16 units would put the
//! caret in the wrong place if the worker did not convert.
//!
//! A test skips itself when its server is not installed, and says so. `LATHE_REQUIRE_LSP` lists the
//! languages that must run (`LATHE_REQUIRE_LSP=rust,typescript,python,go`, or `1` for rust), and a
//! missing server for one of them fails the run rather than passing quietly.
//!
//! With `LATHE_TEST_DOWNLOADS=1` the tests ignore the servers installed here and use only the ones
//! lathe downloads into an empty folder, so the run proves what a user with nothing installed gets.

use std::{
    fs,
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

use lathe_lsp::{
    Doc, Found, LspError, Navigation, ServerSpec, Store, Workers, find_program, find_root, language_id, search_dirs,
    server_for,
};
use lsp_types::{DiagnosticSeverity, Position};

/// Servers index a project before they answer, so the waits are generous.
const READY: Duration = Duration::from_secs(120);
const ASK: Duration = Duration::from_secs(60);

/// One language's copy of the program: its files, and where things are in the source file.
struct Fixture {
    language: &'static str,
    /// Files other than the source, such as a manifest, as (name, text).
    project: &'static [(&'static str, &'static str)],
    source: (&'static str, &'static str),
    declaration_line: u32,
    error_line: u32,
    call_line: u32,
}

const RUST: Fixture = Fixture {
    language: "rust",
    project: &[("Cargo.toml", "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")],
    source: (
        "src/lib.rs",
        "pub fn width() -> u32 {\n    7\n}\n\npub fn broken() -> u32 {\n    let text: u32 = \"not a number\";\n    text + \"😀\".len() as u32 * 0 + width()\n}\n",
    ),
    declaration_line: 0,
    error_line: 5,
    call_line: 6,
};

const TYPESCRIPT: Fixture = Fixture {
    language: "typescript",
    project: &[("tsconfig.json", "{ \"compilerOptions\": { \"strict\": true } }\n")],
    source: (
        "fixture.ts",
        "export function width(): number {\n  return 7;\n}\n\nexport function broken(): number {\n  const text: number = \"not a number\";\n  return text + \"😀\".length * 0 + width();\n}\n",
    ),
    declaration_line: 0,
    error_line: 5,
    call_line: 6,
};

const PYTHON: Fixture = Fixture {
    language: "python",
    project: &[("pyproject.toml", "[project]\nname = \"fixture\"\nversion = \"0.1.0\"\n")],
    source: (
        "fixture.py",
        "def width() -> int:\n    return 7\n\n\ndef broken() -> int:\n    text: int = \"not a number\"\n    return text + len(\"😀\") * 0 + width()\n",
    ),
    declaration_line: 0,
    error_line: 5,
    call_line: 6,
};

const GO: Fixture = Fixture {
    language: "go",
    project: &[("go.mod", "module example.com/fixture\n\ngo 1.21\n")],
    source: (
        "fixture.go",
        "package fixture\n\nfunc width() int {\n\treturn 7\n}\n\nfunc broken() int {\n\tvar text int = \"not a number\"\n\treturn text + len(\"😀\")*0 + width()\n}\n",
    ),
    declaration_line: 2,
    error_line: 7,
    call_line: 8,
};

#[test]
fn rust() {
    check(&RUST);
}

#[test]
fn typescript() {
    check(&TYPESCRIPT);
}

#[test]
fn python() {
    check(&PYTHON);
}

#[test]
fn go() {
    check(&GO);
}

const JAVA: Fixture = Fixture {
    language: "java",
    project: &[(
        "pom.xml",
        "<project><modelVersion>4.0.0</modelVersion><groupId>f</groupId><artifactId>f</artifactId><version>1</version></project>\n",
    )],
    source: (
        "src/main/java/Fixture.java",
        "public class Fixture {\n    static int width() {\n        return 7;\n    }\n\n    static int broken() {\n        int text = \"not a number\";\n        return text + \"😀\".length() * 0 + width();\n    }\n}\n",
    ),
    declaration_line: 1,
    error_line: 6,
    call_line: 7,
};

#[test]
fn java() {
    check(&JAVA);
}

/// Two files of one project share one server, and a file of another project gets its own.
#[test]
fn one_server_per_project() {
    let spec = server_for("rust").expect("rust has a server");
    if !runs("rust", spec) {
        return;
    }
    let base = std::env::temp_dir().join(format!("lathe-lsp-pool-{}", std::process::id()));
    let manifest = "[package]\nname = \"p\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
    for project in ["one", "two"] {
        write(&base.join(project).join("Cargo.toml"), manifest);
        write(&base.join(project).join("src/lib.rs"), "mod other;\n");
        write(&base.join(project).join("src/other.rs"), "");
    }
    let workers = workers();
    let lib = workers.for_file(&base.join("one/src/lib.rs"), &say).expect("a server for the first file");
    let other = workers.for_file(&base.join("one/src/other.rs"), &say).expect("the same server for its neighbour");
    let elsewhere = workers.for_file(&base.join("two/src/lib.rs"), &say).expect("a server for the other project");
    assert!(lib.same_server(&other), "one project, one server");
    assert!(!lib.same_server(&elsewhere), "another project, another server");
    assert_eq!(lib.root(), base.join("one").canonicalize().unwrap().as_path());
    drop((lib, other, elsewhere, workers));
    fs::remove_dir_all(&base).ok();
}

/// Whether the tests use only the servers lathe downloads.
fn downloading() -> bool {
    std::env::var("LATHE_TEST_DOWNLOADS").is_ok_and(|v| v == "1")
}

/// The servers to test: the ones installed here, or with `LATHE_TEST_DOWNLOADS=1` the ones lathe
/// downloads into a folder that starts empty. Go is still the one installed here, since gopls is
/// built with it.
fn workers() -> Workers {
    let store = if downloading() {
        let dir = std::env::temp_dir().join(format!("lathe-lsp-downloads-{}", std::process::id()));
        let go = find_program("go").and_then(|go| go.parent().map(Path::to_path_buf));
        Store::new(dir, go.into_iter().collect(), false)
    } else {
        Store::new(std::env::temp_dir().join("lathe-lsp-no-downloads"), search_dirs(), true)
    };
    Workers::new(store, READY, ASK)
}

/// Prints each download, so a slow first run says why.
fn say(line: String) {
    eprintln!("{line}");
}

/// Whether this language's test can run here; if not, it says why and must not be required.
fn runs(language: &str, spec: &ServerSpec) -> bool {
    let why = match downloading() {
        true => spec.download.is_none().then_some("lathe does not download it"),
        false => find_program(spec.program).is_none().then_some("it is not installed"),
    };
    if let Some(why) = why {
        assert!(!required(language), "{language} is required but {} cannot run: {why}", spec.program);
        eprintln!("skipped: {} cannot run, since {why}, so {language} is unproven here", spec.program);
    }
    why.is_none()
}

/// Whether `LATHE_REQUIRE_LSP` says this language must run.
fn required(language: &str) -> bool {
    std::env::var("LATHE_REQUIRE_LSP")
        .is_ok_and(|v| (v == "1" && language == "rust") || v.split(',').any(|l| l.trim() == language))
}

/// The character column where `width` starts on a line.
fn column_of_width(line: &str) -> u32 {
    let byte = line.rfind("width").expect("the line names width");
    line[..byte].chars().count() as u32
}

/// Asks the worker one question and waits for its answer.
fn ask<T: Send + 'static>(send: impl FnOnce(lathe_lsp::Reply<T>)) -> Result<T, LspError> {
    let (tx, rx) = mpsc::channel();
    send(Box::new(move |answer| drop(tx.send(answer))));
    rx.recv_timeout(ASK + Duration::from_secs(5)).map_err(|_| LspError::Timeout)?
}

/// Asks until `good` accepts an answer. A server answers at once while it loads the project, often
/// with nothing, so an early empty answer means "not ready", not "nowhere".
fn once_ready<T: std::fmt::Debug>(what: &str, mut question: impl FnMut() -> Result<T, LspError>, good: impl Fn(&T) -> bool) -> T {
    let deadline = Instant::now() + READY;
    let mut last = None;
    while Instant::now() < deadline {
        match question() {
            Ok(answer) if good(&answer) => return answer,
            other => last = Some(format!("{other:?}")),
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    panic!("{what}: no good answer in {READY:?}; the last was {}", last.unwrap_or_default());
}

fn check(fixture: &Fixture) {
    let spec = server_for(fixture.language).expect("lathe knows a server for the language");
    if !runs(fixture.language, spec) {
        return;
    }

    let dir = std::env::temp_dir().join(format!("lathe-lsp-{}-{}", fixture.language, std::process::id()));
    for (name, text) in fixture.project {
        write(&dir.join(name), text);
    }
    let path = dir.join(fixture.source.0);
    let text = fixture.source.1.to_string();
    write(&path, &text);
    assert_eq!(language_id(&path), Some(fixture.language));
    let root = find_root(&path, spec.root_markers);
    assert_eq!(root, dir, "the root is the directory with the project file");

    let workers = workers();
    let worker = workers.for_file(&path, &say).expect("the server starts and shakes hands");
    eprintln!("{}: {}", fixture.language, worker.name());
    let doc = |text: &str| Doc { path: path.clone(), text: text.to_string() };
    let line = |text: &str, n: u32| text.split('\n').nth(n as usize).unwrap_or("").to_string();

    // From the call, to the declaration.
    let call = Position { line: fixture.call_line, character: column_of_width(&line(&text, fixture.call_line)) + 1 };
    let navigation = once_ready(
        "definition from the call",
        || ask(|r| worker.navigate(doc(&text), call, r)),
        |n: &Navigation| n.found == Found::Definition && !n.targets.is_empty(),
    );
    assert_eq!(navigation.targets[0].range.start.line, fixture.declaration_line, "it lands on the declaration");

    // On the declaration, to the call.
    let declaration =
        Position { line: fixture.declaration_line, character: column_of_width(&line(&text, fixture.declaration_line)) + 1 };
    let uses = once_ready(
        "references from the declaration",
        || ask(|r| worker.navigate(doc(&text), declaration, r)),
        |n: &Navigation| !n.targets.is_empty(),
    );
    assert_eq!(uses.found, Found::References, "on its declaration, a symbol leads to its uses");
    let target = &uses.targets[0];
    assert_eq!((target.range.start.line, target.range.start.character), (call.line, call.character - 1), "the call, in characters");
    assert!(target.line_text.contains("width()"), "the row shows the call's line: {}", target.line_text);

    // The type error.
    let diagnostics = once_ready(
        "the type error",
        || ask(|r| worker.diagnostics(doc(&text), r)),
        |found: &Vec<lsp_types::Diagnostic>| {
            found.iter().any(|d| d.severity == Some(DiagnosticSeverity::ERROR) && d.range.start.line == fixture.error_line)
        },
    );
    assert!(!diagnostics.is_empty());

    // Hover.
    let hover = ask(|r| worker.hover(doc(&text), call, r)).expect("the server answers hover");
    let shown = format!("{:?}", hover.expect("the server describes width").contents);
    assert!(shown.contains("width"), "the hover names the function: {shown}");

    // Two lines on top: every answer moves down with the text.
    let comment = match fixture.language {
        "python" => "#",
        _ => "//",
    };
    let edited = format!("{comment} one\n{comment} two\n{text}");
    let moved_call = Position { line: call.line + 2, ..call };
    let navigation = once_ready(
        "definition after the edit",
        || ask(|r| worker.navigate(doc(&edited), moved_call, r)),
        |n: &Navigation| n.found == Found::Definition && !n.targets.is_empty(),
    );
    assert_eq!(navigation.targets[0].range.start.line, fixture.declaration_line + 2, "the answer is about the edited text");

    drop((worker, workers));
    fs::remove_dir_all(&dir).ok();
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).expect("the fixture directory is writable");
    fs::write(path, text).expect("the fixture is written");
}
