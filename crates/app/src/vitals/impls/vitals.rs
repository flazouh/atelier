use atelier_agents::{
    labs::Lab,
    usage::{ClaudeUsage, CodexUsage, OpenRouterUsage, Reading, UsageSource},
};
use atelier_project::Project;
use atelier_ui::{Gauge, GaugeState, ProviderGauge, StatusBar, SystemLoad, Work, menu::Lead};
use gpui_kit::{Context, IntoElement, Render, SharedString, Window};

use super::super::{consts::HISTORY, structs::Vitals, traits::LoadProbe, types::Handler};

impl Vitals {
    pub fn new(probe: Box<dyn LoadProbe>) -> Self {
        Self { probe, load: None, providers: Vec::new(), work: Work::default(), columns: (None, None), version: None, on_version: None }
    }

    #[cfg(test)]
    pub fn load(&self) -> Option<&SystemLoad> {
        self.load.as_ref()
    }

    #[cfg(test)]
    pub fn providers(&self) -> &[ProviderGauge] {
        &self.providers
    }

    /// The providers to ask, each with its mark: Claude and Codex wherever they are signed in, and OpenRouter when the
    /// reader gave a key.
    pub fn sources(openrouter_key: Option<String>) -> Vec<(Lead, Box<dyn UsageSource>)> {
        let mut sources: Vec<(Lead, Box<dyn UsageSource>)> = vec![
            (Lead::of(Lab::Anthropic.mark()), Box::new(ClaudeUsage)),
            (Lead::of(Lab::OpenAi.mark()), Box::new(CodexUsage::new("codex"))),
        ];
        if let Some(key) = openrouter_key {
            sources.push((Lead::of(Lab::OpenRouter.mark()), Box::new(OpenRouterUsage::new(key))));
        }
        sources
    }

    /// Asks each of `sources` in turn, on `project`'s host, at Unix time `now`. It blocks on processes and the network.
    pub fn ask(
        sources: &[(Lead, Box<dyn UsageSource>)],
        project: &dyn Project,
        now: i64,
    ) -> Vec<(String, Lead, Result<Reading, String>)> {
        sources.iter().map(|(lead, source)| (source.name().to_string(), lead.clone(), source.read(project, now))).collect()
    }

    /// The version at the left of the bar, and what a press on it does.
    pub fn set_version(&mut self, version: impl Into<SharedString>, on_press: Handler) {
        self.version = Some(version.into());
        self.on_version = Some(on_press);
    }
    /// Where the bar's cards stand: under the sidebar, under the right pane. Whether it changed is the answer.
    pub fn set_columns(&mut self, lead: Option<f32>, tail: Option<f32>) -> bool {
        std::mem::replace(&mut self.columns, (lead, tail)) != (lead, tail)
    }

    /// What the agents do now. Whether it changed is the answer, so the caller draws again only then.
    pub fn set_work(&mut self, work: Work) -> bool {
        std::mem::replace(&mut self.work, work) != work
    }

    /// Reads the machine once more, and keeps the processor's last [`HISTORY`] samples.
    pub fn sample(&mut self) {
        let cpu = self.probe.cpu().clamp(0., 1.);
        let (memory_used, memory_total) = self.probe.memory();
        let app_memory = self.probe.app_memory();
        let mut cpu_history = self.load.take().map(|load| load.cpu_history).unwrap_or_default();
        cpu_history.push(cpu);
        cpu_history.drain(..cpu_history.len().saturating_sub(HISTORY));
        self.load = Some(SystemLoad { cpu, cpu_history, memory_used, memory_total, app_memory });
    }

    /// Takes what `name` said now. A reading replaces the old one. A failure keeps the old numbers, told as old; with
    /// none to keep, the provider shows only if it has shown before, so a machine without Codex has no Codex chip.
    pub fn settle(&mut self, name: &str, lead: Lead, reading: Result<Reading, String>) {
        let at = self.providers.iter().position(|p| p.name.as_ref() == name);
        let fresh = match reading {
            Ok(reading) => Some(gauges(name, lead, reading)),
            Err(why) => match at {
                Some(at) => Some(failed(self.providers[at].clone(), why)),
                None if ClaudeUsage::proves_account(&why) => Some(failed(ProviderGauge::new(name.to_string(), lead), why)),
                None => None,
            },
        };
        match (fresh, at) {
            (Some(fresh), Some(at)) => self.providers[at] = fresh,
            (Some(fresh), None) => self.providers.push(fresh),
            (None, _) => {}
        }
    }
}

fn gauges(name: &str, lead: Lead, reading: Reading) -> ProviderGauge {
    let mut provider = ProviderGauge::new(name.to_string(), lead);
    for window in reading.windows {
        provider = provider.gauge(Gauge::new(window.label, window.used, window.resets_in));
    }
    match reading.note {
        Some(note) => provider.note(note),
        None => provider,
    }
}

/// The last numbers, old; or, with no numbers to show, only why.
fn failed(last: ProviderGauge, why: String) -> ProviderGauge {
    let state = if last.gauges.is_empty() { GaugeState::Unavailable(why.into()) } else { GaugeState::Stale };
    last.state(state)
}

impl Render for Vitals {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let bar = StatusBar::new("status-bar")
            .load(self.load.clone())
            .work(self.work)
            .providers(self.providers.clone())
            .columns(self.columns.0, self.columns.1)
            .version(self.version.clone());
        match self.on_version.clone() {
            Some(press) => bar.on_version(move |window, cx| press(window, cx)),
            None => bar,
        }
    }
}
