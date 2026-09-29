use super::*;

#[test]
fn a_host_is_named_as_builds_are() {
    assert_eq!(Platform::from_uname("Linux x86_64").unwrap().name(), "linux-x86_64");
    assert_eq!(Platform::from_uname("Darwin arm64\n").unwrap().name(), "darwin-aarch64");
    assert_eq!(Platform::from_uname(""), None);
    assert_eq!(remote_binary("0.1.0"), ".cache/lathe/remote/0.1.0/lathe-remote");
}

#[test]
fn the_config_offers_named_hosts_not_patterns() {
    let config = "Host *\n  ServerAliveInterval 30\nHost hp-agent hp\n  HostName 10.0.0.2\nhost air\nHost !bad *.corp pro\n# Host commented\n";
    assert_eq!(hosts_in_config(config), ["hp-agent", "hp", "air", "pro"]);
}
