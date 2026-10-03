//! Writes string keys in every locale with the model that translates (docs/i18n.md).
//!
//! One key: `atelier-translate <folder> <key> <english> <note> [--dry-run]`
//!
//! Many: `atelier-translate <folder> --batch <file> [--jobs N] [--force]`, where each line of the file is
//! `key<TAB>english<TAB>note`. A blank line or one that starts with `#` is skipped, and so is a key whose file is there,
//! unless `--force`. The keys are asked for at the same time, `N` at once (8 unless said).
//!
//! A key's file lands at `<folder>/<key>.rs`. `OPENROUTER_API_KEY` holds the key. With `--dry-run` nothing is sent:
//! the request is printed.

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use atelier_i18n::translate::{self, Key};
use serde_json::Value;

const ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";

struct Job {
    name: String,
    english: String,
    note: String,
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |args: &mut Vec<String>, name: &str| args.iter().position(|a| a == name).map(|at| args.remove(at)).is_some();
    let option = |args: &mut Vec<String>, name: &str| {
        let at = args.iter().position(|a| a == name)?;
        args.remove(at);
        (at < args.len()).then(|| args.remove(at))
    };
    let dry = flag(&mut args, "--dry-run");
    let force = flag(&mut args, "--force");
    let batch = option(&mut args, "--batch");
    let jobs: usize = option(&mut args, "--jobs").and_then(|n| n.parse().ok()).unwrap_or(8).max(1);
    let usage = || {
        eprintln!("usage: atelier-translate <folder> <key> <english> <note> [--dry-run]\n       atelier-translate <folder> --batch <file> [--jobs N] [--force]");
        ExitCode::from(2)
    };
    let (folder, list) = match (args.as_slice(), &batch) {
        ([folder], Some(file)) => match read_jobs(file) {
            Ok(list) => (folder.clone(), list),
            Err(why) => {
                eprintln!("{file}: {why}");
                return ExitCode::FAILURE;
            }
        },
        ([folder, name, english, note], None) => (folder.clone(), vec![Job { name: name.clone(), english: english.clone(), note: note.clone() }]),
        _ => return usage(),
    };
    if dry {
        for job in &list {
            let key = Key { name: &job.name, english: &job.english, note: &job.note };
            if !translate::valid_name(&job.name) {
                eprintln!("{}: {}", job.name, translate::Failure::Name);
                return ExitCode::FAILURE;
            }
            let (system, user) = translate::prompt(&key);
            println!("{}", translate::request_body(&system, &user, &[]));
        }
        return ExitCode::SUCCESS;
    }
    let Ok(token) = std::env::var("OPENROUTER_API_KEY") else {
        eprintln!("OPENROUTER_API_KEY is not set");
        return ExitCode::FAILURE;
    };
    let todo: Vec<&Job> = list.iter().filter(|j| force || !Path::new(&folder).join(format!("{}.rs", j.name)).exists()).collect();
    println!("{} to write, {} already there", todo.len(), list.len() - todo.len());
    let next = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(todo.len()) {
            scope.spawn(|| {
                while let Some(job) = todo.get(next.fetch_add(1, Ordering::Relaxed)) {
                    match run(job, &folder, &token) {
                        Ok(path) => println!("wrote {}", path.display()),
                        Err(why) => {
                            failed.fetch_add(1, Ordering::Relaxed);
                            eprintln!("{}: {why}", job.name);
                        }
                    }
                }
            });
        }
    });
    match failed.load(Ordering::Relaxed) {
        0 => ExitCode::SUCCESS,
        n => {
            eprintln!("{n} failed");
            ExitCode::FAILURE
        }
    }
}

fn read_jobs(file: &str) -> Result<Vec<Job>, String> {
    let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|(n, line)| match line.split('\t').collect::<Vec<_>>().as_slice() {
            [name, english, note] => Ok(Job { name: name.to_string(), english: english.to_string(), note: note.to_string() }),
            _ => Err(format!("line {}: expected key, English and note, separated by tabs", n + 1)),
        })
        .collect()
}

fn run(job: &Job, folder: &str, token: &str) -> Result<PathBuf, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(Duration::from_secs(300))).build().into();
    let mut send = |body: &Value| -> Result<Value, String> {
        let mut response = agent
            .post(ENDPOINT)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("X-Title", "Atelier")
            .send(body.to_string())
            .map_err(|e| e.to_string())?;
        let status = response.status();
        let text = response.body_mut().read_to_string().map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("{status}: {}", text.chars().take(400).collect::<String>()));
        }
        serde_json::from_str(&text).map_err(|e| e.to_string())
    };
    let key = Key { name: &job.name, english: &job.english, note: &job.note };
    let source = translate::translate(&key, &mut send).map_err(|f| f.to_string())?;
    let path = PathBuf::from(folder).join(format!("{}.rs", job.name));
    std::fs::create_dir_all(folder).and_then(|()| std::fs::write(&path, source)).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}
