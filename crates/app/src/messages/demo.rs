//! A seeded `MemoryMessaging` for checking the Messages screen before a real chat account is connected. It exists only in a
//! debug build, and only when `ATELIER_DEMO_MESSAGING` is set to `1`: a release build does not have this module, and a
//! debug build without the variable registers nothing.
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use atelier_capabilities::{
    Actor, CapError, CapResult, Ref, Subscription,
    messaging::{
        Attachment, AttachmentKind, Channel, ChannelKind, ChannelQuery, Event, Filter,
        MemoryMessaging, Message, MessagingCapabilities, MessagingProvider, NewMessage, Page,
        Workspace,
    },
};

use crate::capability_hub::CapabilityHub;

/// The environment variable that asks for the demo account.
pub const VARIABLE: &str = "ATELIER_DEMO_MESSAGING";

/// What a value of the variable asks for: the seeded account, or the seeded account failing in one of the ways the screen has an
/// answer for, so each answer can be looked at.
fn asked(value: Option<&str>) -> Option<Arc<dyn MessagingProvider>> {
    let failing = |error: CapError, channels_too: bool| {
        Some(Arc::new(Failing {
            inner: seeded(),
            error,
            channels_too,
        }) as Arc<dyn MessagingProvider>)
    };
    match value.map(str::trim)? {
        "1" | "true" | "yes" => Some(Arc::new(seeded())),
        // The channels are listed and then the connection drops.
        "offline" => failing(CapError::Offline, false),
        "rate-limited" => failing(
            CapError::RateLimited {
                retry_after_ms: 30_000,
            },
            false,
        ),
        "error" => failing(
            CapError::Provider {
                code: "demo".into(),
                message: "the demo provider failed".into(),
            },
            false,
        ),
        "signed-out" => failing(CapError::NotSignedIn, true),
        _ => None,
    }
}

/// Registers the demo account in `hub` when `value` asks for it. `true` when it did.
pub fn register(hub: &CapabilityHub, value: Option<&str>) -> bool {
    match asked(value) {
        Some(provider) => {
            hub.add_messaging(provider);
            true
        }
        None => false,
    }
}

/// The seeded account that answers `error` to what it is asked to read and to send, from the first channel list on when
/// `channels_too`, else from the first history on.
struct Failing {
    inner: MemoryMessaging,
    error: CapError,
    channels_too: bool,
}

impl MessagingProvider for Failing {
    fn provider(&self) -> &str {
        self.inner.provider()
    }

    fn account(&self) -> &str {
        self.inner.account()
    }

    fn capabilities(&self) -> MessagingCapabilities {
        self.inner.capabilities()
    }

    fn whoami(&self) -> CapResult<Actor> {
        self.inner.whoami()
    }

    fn workspace(&self) -> CapResult<Workspace> {
        self.inner.workspace()
    }

    fn channels(&self, query: &ChannelQuery) -> CapResult<Page<Channel>> {
        match self.channels_too {
            true => Err(self.error.clone()),
            false => self.inner.channels(query),
        }
    }

    fn history(&self, _: &Ref, _: Option<&str>, _: Option<u32>) -> CapResult<Page<Message>> {
        Err(self.error.clone())
    }

    fn thread(&self, _: &Ref, _: Option<&str>) -> CapResult<Page<Message>> {
        Err(self.error.clone())
    }

    fn send(&self, _: &NewMessage, _: &Actor) -> CapResult<Message> {
        Err(self.error.clone())
    }

    fn subscribe(&self, _: &Filter) -> CapResult<Subscription<Event>> {
        Err(self.error.clone())
    }
}

/// [`register`] with the variable as the environment has it.
pub fn register_from_env(hub: &CapabilityHub) -> bool {
    register(hub, std::env::var(VARIABLE).ok().as_deref())
}

/// An account of three channels and about a dozen messages: a reply, a reaction, a file and a message from an agent.
/// The page holds six, and the first channel has eight top-level messages, so the screen has an older page to load.
pub fn seeded() -> MemoryMessaging {
    const MINUTE: i64 = 60_000;
    let real = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64)
    };
    // The clock is set before each message, so the messages are spread over the last hours. Zero means the real time, which
    // the account has from the end of this function on: a message the reader sends is sent now.
    let now = Arc::new(AtomicI64::new(0));
    let clock = now.clone();
    let demo = MemoryMessaging::new("demo")
        .with_me("Alex")
        .with_page_max(6)
        .with_clock(move || match clock.load(Ordering::Relaxed) {
            0 => real(),
            set => set,
        });
    let start = real() - 5 * 60 * MINUTE;
    let at = |minutes: i64| now.store(start + minutes * MINUTE, Ordering::Relaxed);
    let (alex, ana, ben) = (
        demo.whoami().expect("the demo person"),
        Actor::person("ana", "Ana"),
        Actor::person("ben", "Ben"),
    );
    let agent = Actor::agent("claude", "Claude", &alex.id);

    let general = demo.add_channel("general", ChannelKind::Public);
    let design = demo.add_channel("design-review", ChannelKind::Private);
    let direct = demo.add_channel("Ana", ChannelKind::Dm);
    let say = |channel: &atelier_capabilities::Ref, by: &Actor, text: &str| {
        demo.send(&NewMessage::to(channel, text), by)
            .expect("a demo message")
    };

    at(0);
    say(&general, &ana, "Morning! The **release branch** is cut.");
    at(4);
    say(&general, &ben, "Nice. I will run the smoke tests now.");
    at(9);
    say(
        &general,
        &ana,
        "Notes for today are in the doc, see [the plan](https://example.com/plan).",
    );
    at(15);
    say(&general, &ben, "Standup in ten minutes.");
    at(30);
    let question = say(
        &general,
        &ben,
        "Does anyone know why the *gateway* port changes on every start?",
    );
    at(33);
    demo.send(
        &NewMessage::reply(
            &question.reference,
            &general,
            "It binds port 0 and the OS picks one. The grant file has the real one.",
        ),
        &ana,
    )
    .expect("a reply");
    at(35);
    demo.send(
        &NewMessage::reply(&question.reference, &general, "That explains it, thanks."),
        &ben,
    )
    .expect("a reply");
    at(40);
    let report = say(&general, &ana, "Here is the report from the last run.");
    demo.attach(
        &report.reference,
        Attachment {
            id: "f1".into(),
            name: "smoke-report.pdf".into(),
            mime: Some("application/pdf".into()),
            size: Some(184_320),
            url: "https://example.com/smoke-report.pdf".into(),
            kind: AttachmentKind::File,
        },
    )
    .expect("a file");
    at(45);
    let shipped = say(&general, &alex, "Shipping the fix to the gateway today.");
    demo.react(&shipped.reference, "thumbsup", true, &ana)
        .expect("a reaction");
    demo.react(&shipped.reference, "thumbsup", true, &ben)
        .expect("a reaction");
    demo.react(&shipped.reference, "tada", true, &alex)
        .expect("a reaction");
    at(90);
    say(
        &general,
        &agent,
        "I ran the checks on the release branch. All of them pass.",
    );

    at(60);
    say(
        &design,
        &ana,
        "The new sidebar spacing looks right at every zoom.",
    );
    at(70);
    say(&design, &alex, "Good. Ship it after the review.");

    at(120);
    say(
        &direct,
        &ana,
        "Can you look at my pull request when you have a minute?",
    );
    demo.set_unread(&direct, 1).expect("an unread count");
    now.store(0, Ordering::Relaxed);
    demo
}

#[cfg(test)]
mod tests;
