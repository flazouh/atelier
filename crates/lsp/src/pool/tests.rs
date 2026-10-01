use super::*;

fn workers() -> Workers {
    let store = Store::new(std::env::temp_dir().join("atelier-pool-tests"), vec![], true);
    let project = std::sync::Arc::new(atelier_project::LocalProject::open("/").unwrap());
    Workers::new(project, store, Duration::from_secs(1), Duration::from_secs(1))
}

#[test]
fn a_file_with_no_known_language_says_so() {
    let error = workers().for_file(Path::new("/tmp/notes.xyz"), &|_| {}).err().expect("no server");
    assert_eq!(error.to_string(), "no language server for .xyz files");
}

#[test]
fn a_known_server_that_is_missing_names_its_install_command() {
    // Nobody installs a program by this name, so the lookup fails the same way on every machine.
    let spec = crate::ServerSpec {
        name: "nothing",
        program: "atelier-no-such-language-server",
        args: &[],
        language_ids: &[],
        root_markers: &[],
        install: "install it",
        download: None,
        initialization_options: |_, _| None,
    };
    assert!(crate::find_program(spec.program).is_none());
    let message = NoServer::NotInstalled { server: spec.name, install: spec.install }.to_string();
    assert_eq!(message, "nothing is not installed: install it");
}
