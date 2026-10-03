//! The sign-in of a session's agent, when it has none. The agent runs headless and cannot run `/login`, so atelier
//! runs the agent's own sign-in command (`Backend::sign_in`) on this machine, and once it is done the agent starts
//! again on the same session and the message it refused goes again. A project on another host cannot be signed in
//! from here: the notice says where to do it.

use atelier_agents::subprocess;
use atelier_ui::SignInState;
use gpui_kit::{AppContext, Context};

use super::{AgentSession, types::SignIn};

const BY_HAND: &str = "Atelier cannot sign this agent in. Sign in by hand, then send your message again.";

impl AgentSession {
    /// What the notice over the composer shows, or `None` when the agent needs no sign-in and none is going on.
    pub fn sign_in_state(&self) -> Option<SignInState> {
        let elsewhere = || self.project.host().map(|host| SignInState::Elsewhere(host.to_string().into()));
        match &self.signing_in {
            SignIn::Waiting => Some(SignInState::Waiting),
            SignIn::Failed(why) => Some(SignInState::Failed(why.clone())),
            SignIn::Elsewhere => elsewhere(),
            SignIn::Idle if self.conversation.signed_out() => Some(elsewhere().unwrap_or(SignInState::Ready)),
            SignIn::Idle => None,
        }
    }

    /// The account the notice names: a named one the reader made, else none.
    pub fn sign_in_account(&self) -> Option<String> {
        crate::providers::named_account(self.provider.as_ref())
    }

    /// Opens the agent's sign-in, off the UI thread. When it is done the agent starts again and the message it refused
    /// goes once more.
    pub fn sign_in(&mut self, cx: &mut Context<Self>) {
        if self.signing_in == SignIn::Waiting {
            return;
        }
        if self.project.host().is_some() {
            self.signing_in = SignIn::Elsewhere;
            return cx.notify();
        }
        let account = crate::providers::sign_in_account(self.provider.as_ref());
        let Some(command) = self.agent.backend.sign_in(&account) else {
            self.signing_in = SignIn::Failed(BY_HAND.into());
            return cx.notify();
        };
        self.signing_in = SignIn::Waiting;
        let project = self.project.clone();
        let signing = cx.background_spawn(async move { subprocess::run(project.as_ref(), &command) });
        self._signing = cx.spawn(async move |this, cx| {
            let outcome = signing.await;
            _ = this.update(cx, |session, cx| session.signed_in(outcome, cx));
        });
        cx.notify();
    }

    /// A turn ended, `ok` when it completed: one that ran shows the agent is signed in, and whatever the notice said is over.
    pub(super) fn turn_ran(&mut self, ok: bool) {
        if ok && self.signing_in != SignIn::Waiting {
            self.signing_in = SignIn::Idle;
        }
    }

    fn signed_in(&mut self, outcome: Result<(), String>, cx: &mut Context<Self>) {
        match outcome {
            Err(why) => self.signing_in = SignIn::Failed(format!("The sign-in did not finish: {why}").into()),
            Ok(()) => {
                self.signing_in = SignIn::Idle;
                self.conversation.signed_in();
                // The agent may hold on to what it found when it started: it starts again, on the same session, with the
                // next message.
                if !self.conversation.working() {
                    self.session = None;
                }
                if let Some(refused) = self.conversation.take_unanswered() {
                    self.send(refused, cx);
                }
            }
        }
        cx.notify();
    }
}
