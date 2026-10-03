use std::{ops::Range, time::Duration};

/// The ways to let words arrive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Way {
    /// Today: the text is replaced, nothing moves.
    Today,
    /// New words fade in; a rewritten word just changes.
    Fade,
    /// The last words are tentative (muted) and firm up to full ink once they are safely behind the head.
    Settle,
    /// Words are released one by one at an even beat, not in the half-second bursts they arrive in, each fading in.
    Paced,
    /// New and rewritten words are wet ink: tinted, drying to full ink over time.
    WetInk,
}

impl Way {
    pub const ALL: [Way; 5] = [Way::Today, Way::Fade, Way::Settle, Way::Paced, Way::WetInk];

    pub fn name(self) -> &'static str {
        match self {
            Way::Today => "A  Today",
            Way::Fade => "B  Fade in",
            Way::Settle => "C  Settle",
            Way::Paced => "D  Paced",
            Way::WetInk => "E  Wet ink",
        }
    }

    pub fn gist(self) -> &'static str {
        match self {
            Way::Today => "The text is replaced each time. The control.",
            Way::Fade => "New words fade in over about 150 ms. Rewrites swap.",
            Way::Settle => "The last 3 words are muted until they are safe; then they firm up.",
            Way::Paced => "A burst of words is released on a steady beat, each fading in.",
            Way::WetInk => "New and rewritten words arrive tinted and dry to ink.",
        }
    }
}

/// One word of the text, and what is drawn of it: `a` is its opacity, `tint` how much of the accent it has.
#[derive(Clone, Debug)]
pub struct Word {
    pub range: Range<usize>,
    pub a: f32,
    pub tint: f32,
    /// When it was last rewritten.
    pub revised: Option<std::time::Instant>,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub way: Way,
    pub text: String,
    pub words: Vec<Word>,
    /// How many words are let out (Paced only; the others show them all).
    pub shown: usize,
    next_release: std::time::Instant,
}

fn split(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                out.push(s..i);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push(s..text.len());
    }
    out
}

impl Track {
    pub fn new(way: Way, now: std::time::Instant) -> Self {
        Self { way, text: String::new(), words: Vec::new(), shown: 0, next_release: now }
    }

    /// The engine said `text`. A word at a place that held another word is rewritten; a place past the old end is new.
    pub fn observe(&mut self, text: &str, now: std::time::Instant) {
        let mut words = Vec::new();
        for (i, range) in split(text).into_iter().enumerate() {
            let fresh = |a| Word { range: range.clone(), a, tint: 0., revised: None };
            match self.words.get(i) {
                Some(old) if self.text[old.range.clone()] == text[range.clone()] => words.push(Word { range, ..old.clone() }),
                Some(old) => {
                    let mut w = Word { range, ..old.clone() };
                    w.revised = Some(now);
                    if self.way == Way::WetInk {
                        w.tint = 1.;
                    }
                    words.push(w);
                }
                None => {
                    let mut w = fresh(if self.way == Way::Today { 1. } else { 0. });
                    if self.way == Way::WetInk {
                        w.tint = 1.;
                    }
                    words.push(w)
                }
            }
        }
        self.words = words;
        self.text = text.to_string();
        self.shown = self.shown.min(self.words.len());
        if self.way != Way::Paced {
            self.shown = self.words.len();
        }
    }

    /// Everything seen so far is old news: full ink, all let out.
    pub fn settle(&mut self) {
        self.shown = self.words.len();
        for w in &mut self.words {
            w.a = if self.way == Way::Settle { 1. } else { 1. };
            w.tint = 0.;
            w.revised = None;
        }
    }

    /// Moves every word `dt` on.
    pub fn step(&mut self, dt: Duration, now: std::time::Instant) {
        let dt = dt.as_secs_f32();
        let ease = |tau: f32| 1. - (-dt / tau).exp();
        // Several words may be due in one step when frames are slow: the beat is kept, not the frame rate.
        while self.way == Way::Paced && self.shown < self.words.len() && now >= self.next_release {
            let backlog = self.words.len() - self.shown;
            self.shown += 1;
            // A burst takes at most about 300 ms to come out, and no word waits longer than 140 ms for the one before.
            let gap = (0.30 / backlog as f32).clamp(0.04, 0.14);
            self.next_release = self.next_release.max(now - Duration::from_millis(200)) + Duration::from_secs_f32(gap);
        }
        let n = self.words.len();
        let shown = if self.way == Way::Paced { self.shown } else { n };
        for (i, w) in self.words.iter_mut().enumerate() {
            let from_end = n - 1 - i;
            let (target, tau) = match self.way {
                Way::Today => (1., 0.001),
                Way::Fade | Way::Paced | Way::WetInk => (if i < shown { 1. } else { 0. }, 0.05),
                Way::Settle => {
                    let rewritten = w.revised.is_some_and(|at| now.saturating_duration_since(at) < Duration::from_millis(700));
                    (if from_end < 3 || rewritten { 0.45 } else { 1. }, 0.12)
                }
            };
            w.a += (target - w.a) * ease(tau);
            if self.way == Way::WetInk {
                w.tint *= 1. - ease(0.35);
            }
        }
    }

    /// The text to draw: Paced shows only the words let out.
    pub fn shown_text(&self) -> &str {
        let shown = if self.way == Way::Paced { self.shown } else { self.words.len() };
        match shown.checked_sub(1).and_then(|i| self.words.get(i)) {
            Some(last) => &self.text[..last.range.end],
            None => "",
        }
    }
}
