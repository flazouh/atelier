//! What listing a large project costs: 10,000 files in 500 folders, with a .gitignore, as
//! `Project::list` walks it for the app's tree, on a background thread. Target: under 200 ms.
//!     cargo test --release -p atelier-project --test list_bench -- --ignored --nocapture

use std::time::{Duration, Instant};

use atelier_project::{LocalProject, Project};

#[test]
#[ignore]
fn listing_ten_thousand_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
    for folder in 0..500 {
        let at = dir.path().join(format!("src/m{:03}", folder / 20)).join(format!("f{folder:03}"));
        std::fs::create_dir_all(&at).unwrap();
        for file in 0..20 {
            std::fs::write(at.join(format!("file{file:02}.rs")), "fn x() {}\n").unwrap();
        }
    }
    std::fs::create_dir_all(dir.path().join("target/debug")).unwrap();
    for n in 0..2000 {
        std::fs::write(dir.path().join(format!("target/debug/{n}.o")), "").unwrap();
    }
    let project = LocalProject::open(dir.path()).unwrap();
    let mut samples: Vec<Duration> = (0..20)
        .map(|_| {
            let at = Instant::now();
            let entries = project.list().unwrap();
            assert_eq!(entries.iter().filter(|e| !e.dir).count(), 10_001, "10,000 files and the .gitignore; target is ignored");
            at.elapsed()
        })
        .collect();
    samples.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.;
    println!("10,000 files listed: median {:.2} ms, p95 {:.2} ms, target < 200 ms", ms(samples[10]), ms(samples[18]));
}
