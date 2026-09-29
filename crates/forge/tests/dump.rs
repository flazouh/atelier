use lathe_forge::{Forge, PullRef, RepoRef, github::{GitHub, testing::Fixtures}};
#[test]
fn dump() {
    let fx = Fixtures::from_dir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/github")));
    let f = GitHub::with_transport(fx);
    let r = PullRef { repo: RepoRef::new("github.com", "oven-sh", "bun"), number: 44169 };
    let mut p = f.pull(&r).unwrap(); p.body = format!("{} chars", p.body.len());
    println!("{p:#?}");
    for t in f.threads(&r).unwrap() { println!("T {:?} {} {:?} {:?} res={} out={} n={} first={:?}", t.id, t.path, t.line, t.side, t.resolved, t.outdated, t.comments.len(), (t.comments[0].author.clone(), t.comments[0].kind, t.comments[0].body.chars().take(40).collect::<String>())); }
    for c in f.remarks(&r).unwrap() { println!("R {} {:?} {}", c.author, c.kind, c.body.chars().take(30).collect::<String>()); }
    for c in f.checks(&r).unwrap() { println!("C {} {:?} {:?} req={} job={:?} run={:?}", c.name, c.status, c.conclusion, c.required, c.job.as_ref().map(|j| j.id), c.run.as_ref().map(|r| r.workflow.clone())); }
    for x in f.files(&r).unwrap() { println!("F {:?}", x); }
    println!("{:#?}", f.briefs(&r.repo, &[44169, 1, 44032]).unwrap());
    let job = f.job(&lathe_forge::JobRef { repo: r.repo.clone(), id: 109256890058 }).unwrap();
    println!("{job:#?}");
    let log = f.job_log(&job.reference).unwrap(); println!("log starts {:?}", &log[..40]);
}
