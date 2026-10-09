use atelier_capabilities::{CapError, messaging::MessagingProvider};

use super::support::{Fixtures, general, provider};
use crate::{DiscordConfig, DiscordMessaging, Output, RunError, Runner, error_of};

#[test]
fn what_discordcli_says_maps_to_the_errors_of_the_capability() {
    let cases: [(&str, CapError); 9] = [
        (
            r#"{"error":"Not logged in. Run discordcli login."}"#,
            CapError::NotSignedIn,
        ),
        (
            r#"{"error":"Discord session expired. Run discordcli login."}"#,
            CapError::NotSignedIn,
        ),
        (
            "Error: Discord session file is invalid. Run discordcli login.",
            CapError::NotSignedIn,
        ),
        (
            r#"{"error":"DISCORD_TOKEN is empty or invalid."}"#,
            CapError::NotSignedIn,
        ),
        (
            r#"{"error":"Discord rate limit reached. Try again later."}"#,
            CapError::RateLimited {
                retry_after_ms: 10_000,
            },
        ),
        (
            r#"{"error":"Discord returned HTTP 404."}"#,
            CapError::not_found("the channel or message"),
        ),
        (
            r#"{"error":"Discord request failed. Check the network and retry."}"#,
            CapError::Offline,
        ),
        (
            r#"{"error":"Channel must be a valid Discord snowflake ID."}"#,
            CapError::invalid("id"),
        ),
        (
            r#"{"error":"Discord denied access. Check the account and channel permissions."}"#,
            CapError::Provider {
                code: "forbidden".into(),
                message: "Discord denied access. Check the account and channel permissions.".into(),
            },
        ),
    ];
    for (stderr, want) in cases {
        assert_eq!(error_of(stderr), want, "{stderr}");
    }
}

#[test]
fn a_search_that_is_still_indexing_is_not_an_empty_result() {
    let err = error_of(r#"{"error":"Discord search is still indexing. Try again shortly."}"#);
    assert!(matches!(&err, CapError::Provider { code, .. } if code == "search_pending"));
}

#[test]
fn an_unknown_failure_keeps_its_words() {
    let err =
        error_of("Error: This channel belongs to a different server. Check the link or --server.");
    assert_eq!(
        err,
        CapError::Provider {
            code: "discordcli".into(),
            message: "This channel belongs to a different server. Check the link or --server."
                .into()
        }
    );
}

#[test]
fn a_failed_command_reaches_the_caller_as_that_error() {
    let fixtures = Fixtures::the_usual().with(
        "read",
        Output::failed(r#"{"error":"Discord session expired. Run discordcli login."}"#),
    );
    assert_eq!(
        provider(&fixtures)
            .history(&general(), None, None)
            .unwrap_err(),
        CapError::NotSignedIn
    );
}

struct Missing;
impl Runner for Missing {
    fn run(&self, _args: &[String]) -> Result<Output, RunError> {
        Err(RunError::NotInstalled("discordcli".into()))
    }
}

#[test]
fn a_missing_program_says_so() {
    let p = DiscordMessaging::new(Missing, DiscordConfig::new("1100000000000000001"));
    let err = p.whoami().unwrap_err();
    assert!(matches!(&err, CapError::Provider { code, .. } if code == "discordcli_missing"));
}

#[test]
fn output_that_is_not_json_is_a_provider_error_not_a_panic() {
    let fixtures = Fixtures::the_usual().with("whoami", Output::ok("not json"));
    let err = provider(&fixtures).whoami().unwrap_err();
    assert!(matches!(&err, CapError::Provider { code, .. } if code == "bad_output"));
}

/// Needs a signed-in `discordcli` and a server id in `ATELIER_DISCORD_TEST_SERVER`. It only reads: `whoami` and the
/// list of channels. Run it with `cargo test -p atelier-discord -- --ignored`.
#[test]
#[ignore = "reads from the real Discord through discordcli"]
fn live_read_only_whoami_and_channels() {
    let server = std::env::var("ATELIER_DISCORD_TEST_SERVER").expect("a server id");
    let p = DiscordMessaging::new(
        crate::CommandRunner::new("discordcli"),
        DiscordConfig::new(&server),
    );
    let me = p.whoami().expect("whoami");
    assert!(!me.id.is_empty());
    let channels = p
        .channels(&atelier_capabilities::messaging::ChannelQuery::default())
        .expect("channels");
    println!("{} channels", channels.items.len());
}
