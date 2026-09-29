use super::RepoRef;

fn parsed(url: &str) -> Option<(String, String, String)> {
    RepoRef::from_remote(url).map(|r| (r.host, r.owner, r.name))
}

fn is(host: &str, owner: &str, name: &str) -> Option<(String, String, String)> {
    Some((host.into(), owner.into(), name.into()))
}

#[test]
fn the_usual_remote_forms_name_the_same_repository() {
    for url in [
        "https://github.com/oven-sh/bun",
        "https://github.com/oven-sh/bun.git",
        "https://github.com/oven-sh/bun/",
        "https://user:token@github.com/oven-sh/bun.git",
        "git@github.com:oven-sh/bun.git",
        "git@github.com:oven-sh/bun",
        "ssh://git@github.com/oven-sh/bun.git",
        "ssh://git@github.com:22/oven-sh/bun.git",
        "  https://github.com/oven-sh/bun.git\n",
    ] {
        assert_eq!(parsed(url), is("github.com", "oven-sh", "bun"), "{url}");
    }
}

#[test]
fn the_host_is_kept_so_another_forge_can_be_told_apart() {
    assert_eq!(parsed("https://gitlab.com/a/b.git"), is("gitlab.com", "a", "b"));
    assert_eq!(parsed("git@git.example.org:team/tool.git"), is("git.example.org", "team", "tool"));
}

#[test]
fn a_name_may_hold_dots_and_dashes() {
    assert_eq!(parsed("https://github.com/o/my.repo-2.git"), is("github.com", "o", "my.repo-2"));
}

#[test]
fn anything_that_is_not_an_owner_and_a_name_is_none() {
    for url in ["", "not a url", "https://github.com", "https://github.com/o", "https://github.com/o/r/tree/main", "git@github.com:o", "git@github.com:o/r/x", "https:///o/r", "https://github.com//r"] {
        assert_eq!(parsed(url), None, "{url:?}");
    }
}

#[test]
fn the_slug_is_owner_slash_name() {
    assert_eq!(RepoRef::new("github.com", "o", "r").slug(), "o/r");
}
