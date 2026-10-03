use std::sync::{Arc, atomic::AtomicBool};

use atelier_agents::subprocess;
use atelier_ui::SignInState;
use gpui_kit::{AppContext, Context};

use super::super::{enums::SignIn, structs::Run};
use crate::agent_session::AgentSession;

const BY_HAND: &str = "Atelier cannot sign this agent in. Sign in by hand, then send your message again.";

impl AgentSession {
    /// What the notice over the composer shows, or `None` when the agent needs no sign-in and none is going on.
    pub fn sign_in_state(&self) -> Option<SignInState> {
        let elsewhere = || self.project.host().map(|host| SignInState::Elsewhere(host.to_string().into()));
        match &self.signer.state {
            SignIn::Waiting => Some(SignInState::Waiting),
            SignIn::Failed(why) => Some(SignInState::Failed(why.clone())),
            SignIn::Elsewhere => elsewhere(),
            SignIn::Idle if self.conversation.signed_out() => Some(elsewhere().unwrap_or(SignInState::Ready)),
            SignIn::Idle => None,
        }
    }

    /// The account the notice names: a named one the session runs on, else the named one that holds it, else none.
    pub fn sign_in_account(&self) -> Option<String> {
        crate::providers::named_account(self.provider.as_ref()).or_else(|| self.signer.holder.clone())
    }

    /// The account the sign-in is for.
    fn account_to_sign_in(&self) -> String {
        match (&self.provider, &self.signer.holder) {
            (None, Some(holder)) => holder.clone(),
            _ => crate::providers::sign_in_account(self.provider.as_ref()),
        }
    }

    /// Opens the agent's sign-in, off the UI thread. When it is done the agent starts again and the message it refused
    /// goes once more.
    pub fn sign_in(&mut self, cx: &mut Context<Self>) {
        if self.signer.state == SignIn::Waiting {
            return;
        }
        if self.project.host().is_some() {
            self.signer.state = SignIn::Elsewhere;
            return cx.notify();
        }
        let Some(command) = self.agent.backend.sign_in(&self.account_to_sign_in()) else {
            self.signer.state = SignIn::Failed(BY_HAND.into());
            return cx.notify();
        };
        self.signer.state = SignIn::Waiting;
        let project = self.project.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        let running = cx.background_spawn(async move { subprocess::run(project.as_ref(), &command, &stop) });
        let task = cx.spawn(async move |this, cx| {
            let outcome = running.await;
            _ = this.update(cx, |session, cx| session.signed_in(outcome, cx));
        });
        self.signer.run = Some(Run { cancelled, _task: task });
        cx.notify();
    }

    /// Leaves the wait for the browser: the command stops, and the notice offers the sign-in again.
    pub fn cancel_sign_in(&mut self, cx: &mut Context<Self>) {
        if self.signer.state == SignIn::Waiting {
            self.signer.run = None;
            self.signer.state = SignIn::Idle;
            cx.notify();
        }
    }

    /// A turn ended, `ok` when it completed: one that ran shows the agent is signed in, and whatever the notice said is over.
    /// One the agent turned away finds the account that holds the session, when the session has no provider to say.
    pub(in crate::agent_session) fn turn_ran(&mut self, ok: bool, cx: &mut Context<Self>) {
        if ok && self.signer.state != SignIn::Waiting {
            self.signer.state = SignIn::Idle;
        }
        if !ok && self.conversation.signed_out() {
            self.look_for_holder(cx);
        }
    }

    fn look_for_holder(&mut self, cx: &mut Context<Self>) {
        let (None, None, Some(id)) = (&self.provider, &self.signer.looking, self.id.clone()) else { return };
        let (backend, project) = (self.agent.backend.clone(), self.project.clone());
        let finding = cx.background_spawn(async move { backend.session_account(project.as_ref(), &id) });
        self.signer.looking = Some(cx.spawn(async move |this, cx| {
            let holder = finding.await.ok().flatten();
            _ = this.update(cx, |session, cx| {
                session.signer.holder = holder;
                cx.notify();
            });
        }));
    }

    fn signed_in(&mut self, outcome: Result<(), String>, cx: &mut Context<Self>) {
        self.signer.run = None;
        match outcome {
            Err(why) => self.signer.state = SignIn::Failed(format!("The sign-in did not finish: {why}").into()),
            Ok(()) => {
                self.signer.state = SignIn::Idle;
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
