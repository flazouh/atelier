//! Shows every beui component in each state, in light and dark. Run with `cargo run -p beui-gallery`.
//! `GALLERY_STORY=<title>` opens a story and `GALLERY_THEME=light|dark` overrides the system theme, so a script can screenshot them.
//! `GALLERY_REPLAY=1` starts the Agent panel's "Replay session" on launch, so a capture can see the entrances.

use beui::{
    ActiveTheme, AgentText, AgentTextSource, AgentTextStatus, Appearance, Badge, Button, ButtonSize, ButtonVariant,
    CodeBlock, CodeBlockStatus, DiffLine, DiffLineKind, EntranceList, FONT_FAMILY, FileDiff, FileDiffStatus, Icon, IconName, Kbd, MONO_FONT_FAMILY,
    MessageBubble, MessageBubbleAlign, MessageBubbleCollapsible, MessageBubbleGroupSpacing, MessageBubbleVariant,
    PromptAction, PromptInput, PromptInputEvent, PromptModel, Select, Shimmer, Spark, SparkState, Spinner, SubagentRow, TextSize, Thinking,
    ThinkingPhase, ThinkingStyle, Todo, TodoList,
    CodeEditor, Decision, Hunk, HunkReview, HunkState, InlineHunk, InlineReview, StatusTone, TodoStatus, ToolApproval, ToolApprovalStatus, ToolCall, ToolStatus, Tone, message_bubble_group,
    pane_header,
};
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

mod editor_lsp;

use gpui_kit::base::input::InputEvent;

use gpui_kit::{
    AnyElement, App, AppContext, Bounds, Context, ElementId, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds, WindowOptions, div,
    Task, prelude::FluentBuilder, px, size,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Story {
    AgentPanel,
    Colors,
    Typography,
    Icons,
    Spark,
    Buttons,
    Badges,
    Messages,
    Tools,
    Diffs,
    Plan,
    Hunks,
    Inline,
    Editor,
    Select,
    Prompt,
}

impl Story {
    const ALL: [Story; 16] = [
        Story::AgentPanel,
        Story::Colors,
        Story::Typography,
        Story::Icons,
        Story::Spark,
        Story::Buttons,
        Story::Badges,
        Story::Messages,
        Story::Tools,
        Story::Diffs,
        Story::Plan,
        Story::Hunks,
        Story::Inline,
        Story::Editor,
        Story::Select,
        Story::Prompt,
    ];

    fn title(self) -> &'static str {
        match self {
            Story::AgentPanel => "Agent panel",
            Story::Colors => "Colors",
            Story::Typography => "Typography",
            Story::Icons => "Icons",
            Story::Spark => "Spark",
            Story::Buttons => "Buttons",
            Story::Badges => "Badges and keys",
            Story::Messages => "Messages",
            Story::Tools => "Tool calls",
            Story::Diffs => "Diffs and code",
            Story::Plan => "Plan",
            Story::Hunks => "Hunks",
            Story::Inline => "Inline review",
            Story::Editor => "Editor",
            Story::Select => "Select",
            Story::Prompt => "Prompt input",
        }
    }
}

/// beui's preview models: a provider mark per model. The real favicons preview.tsx fetches over the
/// network have no equivalent here, so every model shows the generic `Bot` mark instead.
fn preview_models() -> Vec<PromptModel> {
    vec![
        PromptModel::new("gpt-5.2", "GPT-5.2").icon(IconName::Bot),
        PromptModel::new("claude-sonnet-4", "Claude Sonnet 4").icon(IconName::Bot),
        PromptModel::new("gemini-3.6-flash", "Gemini 3.6 Flash").icon(IconName::Bot),
        PromptModel::new("grok-4.5", "Grok 4.5").icon(IconName::Bot),
        PromptModel::new("mistral-large-3", "Mistral Large 3").icon(IconName::Bot),
    ]
}

/// beui's preview actions, verbatim.
fn preview_actions() -> Vec<PromptAction> {
    vec![
        PromptAction::new("image", "Attach image")
            .description("Add a screenshot or visual reference.")
            .icon(IconName::ImagePlus),
        PromptAction::new("skill", "Use a skill")
            .description("Give the agent a specialized workflow.")
            .icon(IconName::Puzzle),
        PromptAction::new("context", "Add context")
            .description("Include a file with supporting details.")
            .icon(IconName::FileText),
    ]
}

struct Gallery {
    story: Story,
    choice: Option<usize>,
    /// The Hunks story's own file, so Accept and Reject really resolve.
    hunks: Vec<Hunk>,
    /// The Inline review story's buffer: both sides of every hunk, as real text.
    inline: Entity<gpui_kit::component::input::EditorState>,
    /// The hunks still waiting in that buffer.
    inline_hunks: Vec<InlineHunk>,
    /// That buffer's text as the hunks last matched it, so an edit can move them with the rows.
    inline_text: String,
    /// Moves the hunks whenever the user edits that buffer.
    _inline_edits: gpui_kit::Subscription,
    /// The Editor story's buffer, which the user can really type into.
    editor: Entity<gpui_kit::component::input::EditorState>,
    /// The language server behind the Editor story.
    lsp: LspState,
    prompt: Entity<PromptInput>,
    panel_prompt: Entity<PromptInput>,
    /// Below the "Prompt input" story, as preview.tsx's `sent`/`notice` line.
    notice: Option<SharedString>,
    /// When the gallery opened: the start of every thinking row, so their labels age like a real turn.
    started: Instant,
    /// The Agent panel's "Replay session": `None` shows the whole session.
    replay: Option<Replay>,
    /// Counts replays, so each one is a new list whose items all enter again.
    replays: usize,
    _system: [gpui_kit::Subscription; 2],
}

/// How often "Replay session" adds the next item.
const REPLAY_STEP: Duration = Duration::from_millis(400);

struct Replay {
    shown: usize,
    _timer: Task<()>,
}

impl Gallery {
    /// Sends the buffer to the server and shows what it says is wrong. `announce` is the Check button:
    /// it says so in the status line. An edit refreshes quietly. A set lands only if the buffer still
    /// holds the text it was worked out for, so an underline never sits on the wrong line.
    fn pull_diagnostics(&mut self, announce: bool, cx: &mut Context<Self>) {
        let Some(worker) = self.lsp.worker.clone() else { return };
        let text = self.editor.read(cx).value().to_string();
        let answer = editor_lsp::ask(cx, |reply| worker.diagnostics(text.clone(), reply));
        if announce {
            self.lsp.status = "checking…".into();
        }
        self.lsp.checking = cx.spawn(async move |this, cx| {
            let answer = answer.await;
            _ = this.update(cx, |this, cx| {
                if this.editor.read(cx).value().as_ref() != text {
                    return;
                }
                match answer {
                    Ok(diagnostics) => {
                        let first = diagnostics.first().map(|d| d.message.clone()).unwrap_or_default();
                        this.lsp.status = match diagnostics.len() {
                            0 => "the server finds nothing wrong".into(),
                            1 => format!("1 problem: {first}").into(),
                            n => format!("{n} problems, first: {first}").into(),
                        };
                        beui::code_editor::set_diagnostics(&this.editor, diagnostics, cx);
                    }
                    Err(error) => this.lsp.status = format!("{error}").into(),
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    /// Checks the buffer again once typing pauses for [`RECHECK_AFTER`], as Zed does.
    fn schedule_recheck(&mut self, cx: &mut Context<Self>) {
        if self.lsp.worker.is_none() {
            return;
        }
        self.lsp.recheck = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RECHECK_AFTER).await;
            _ = this.update(cx, |this, cx| this.pull_diagnostics(false, cx));
        });
    }

    /// Asks the server where the symbol under the caret is defined and moves the caret there, as F12
    /// does in Zed. A definition in another file is named in the status line; the editor never jumps
    /// to a line of the wrong file.
    fn go_to_definition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(worker) = self.lsp.worker.clone() else { return };
        let here = editor_lsp::document_uri(&worker);
        let (text, position) = {
            let state = self.editor.read(cx);
            (state.value().to_string(), state.cursor_position())
        };
        let answer = editor_lsp::ask(cx, |reply| worker.definition(text, position, reply));
        self.lsp.status = "looking…".into();
        self.lsp.pending = cx.spawn_in(window, async move |this, cx| {
            let answer = answer.await;
            _ = this.update_in(cx, |this, window, cx| {
                this.lsp.status = match answer.map(lathe_lsp::definition_links) {
                    Ok(links) => match links.first() {
                        Some(link) if Some(&link.target_uri) == here.as_ref() => {
                            let start = link.target_selection_range.start;
                            this.editor.update(cx, |state, cx| state.set_cursor_position(start, window, cx));
                            // A server answer is one-based to a reader, so the line is shown as the editor does.
                            format!("defined on line {}", start.line + 1).into()
                        }
                        Some(link) => format!(
                            "defined in {} on line {}",
                            editor_lsp::display_path(&link.target_uri),
                            link.target_selection_range.start.line + 1
                        )
                        .into(),
                        None => "the server knows of no definition there".into(),
                    },
                    Err(error) => format!("{error}").into(),
                };
                cx.notify();
            });
        });
        cx.notify();
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let story = std::env::var("GALLERY_STORY")
            .ok()
            .and_then(|name| Story::ALL.into_iter().find(|s| s.title().eq_ignore_ascii_case(&name)))
            .unwrap_or(Story::AgentPanel);
        let prompt = cx.new(|cx| {
            PromptInput::new(
                "Ask the agent to do something…",
                "Review the current implementation and suggest the next improvement.",
                window,
                cx,
            )
            .models(preview_models())
            .model("gpt-5.2")
            .actions(preview_actions())
        });
        let panel_prompt = cx.new(|cx| {
            let mut input = PromptInput::new("Ask Claude Code", "", window, cx)
                .models(vec![PromptModel::new("sonnet-5", "Sonnet 5")])
                .model("sonnet-5");
            input.set_running(true, cx);
            input
        });
        // The gallery only prints what the input asked for; the app will act on it.
        for input in [&prompt, &panel_prompt] {
            cx.subscribe(input, |_, _, event: &PromptInputEvent, _| println!("prompt: {event:?}")).detach();
        }
        // Mirrors preview.tsx: a submit or an action pick prints a line under the box.
        cx.subscribe(&prompt, |this, _, event: &PromptInputEvent, cx| {
            this.notice = match event {
                PromptInputEvent::Submit(_) => Some("Prompt sent to the selected model.".into()),
                PromptInputEvent::Action(value) => preview_actions()
                    .into_iter()
                    .find(|a| a.value == *value)
                    .map(|a| format!("{} selected.", a.label).into()),
                _ => return,
            };
            cx.notify();
        })
        .detach();
        let _system = beui::watch_system(window, cx);
        let inline = CodeEditor::state("config.rs", INLINE_FILE, window, cx);
        // The user can type anywhere, so the hunks follow the rows they describe.
        let _inline_edits = cx.subscribe(&inline, |this, state, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let text = state.read(cx).value().to_string();
            if text != this.inline_text {
                this.inline_hunks = beui::inline_review::track_edit(&this.inline_hunks, &this.inline_text, &text);
                this.inline_text = text;
                cx.notify();
            }
        });
        let mut gallery =
            Self {
            story,
            choice: None,
            hunks: sample_hunks(),
            inline,
            inline_hunks: inline_fixture(),
            inline_text: INLINE_FILE.to_string(),
            _inline_edits,
            editor: CodeEditor::state("lib.rs", SAMPLE_RUST, window, cx),
            lsp: LspState { status: "rust-analyzer is starting".into(), ..LspState::default() },
            prompt, panel_prompt, notice: None, started: Instant::now(), replay: None, replays: 0, _system };
        if std::env::var("GALLERY_REPLAY").is_ok_and(|v| v == "1") {
            gallery.start_replay(cx);
        }
        gallery.start_lsp(cx);
        gallery
    }

    /// Starts the Editor story's server without holding the window, then hands it to the editor.
    fn start_lsp(&mut self, cx: &mut Context<Self>) {
        let started = editor_lsp::start(SAMPLE_RUST.to_string());
        self.lsp.pending = cx.spawn(async move |this, cx| {
            let started = started.await.unwrap_or_else(|_| Err("the server thread stopped".into()));
            _ = this.update(cx, |this, cx| {
                match started {
                    Ok(editor_lsp::Started { worker, name }) => {
                        let gallery = cx.entity().downgrade();
                        let elsewhere: editor_lsp::Elsewhere = Rc::new(move |params: &lsp_types::ShowDocumentParams, cx: &mut App| {
                            let line = params.selection.map(|r| r.start.line + 1).unwrap_or(1);
                            let status = format!("defined in {} on line {line}", editor_lsp::display_path(&params.uri));
                            _ = gallery.update(cx, |this, cx| {
                                this.lsp.status = status.into();
                                cx.notify();
                            });
                        });
                        editor_lsp::attach(&this.editor, worker.clone(), elsewhere, cx);
                        this.lsp.worker = Some(worker);
                        this.lsp.edits = Some(cx.subscribe(&this.editor, |this, _, event: &InputEvent, cx| {
                            if matches!(event, InputEvent::Change) {
                                this.schedule_recheck(cx);
                            }
                        }));
                        this.pull_diagnostics(false, cx);
                        this.lsp.status = format!("{name} is ready: hold {SECONDARY} and click a symbol, or press F12").into();
                    }
                    Err(error) => this.lsp.status = error.into(),
                }
                cx.notify();
            });
        });
    }

    /// Clears the Agent panel, then adds its items back one by one every [`REPLAY_STEP`].
    fn start_replay(&mut self, cx: &mut Context<Self>) {
        self.replays += 1;
        let timer = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(REPLAY_STEP).await;
                let more = this
                    .update(cx, |g, cx| {
                        let Some(replay) = g.replay.as_mut() else { return false };
                        replay.shown += 1;
                        cx.notify();
                        replay.shown < SESSION_LEN
                    })
                    .unwrap_or(false);
                if !more {
                    break;
                }
            }
        });
        self.replay = Some(Replay { shown: 0, _timer: timer });
        cx.notify();
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let dark = theme.appearance == Appearance::Dark;
        div()
            .flex()
            .flex_col()
            .w(px(220.))
            .h_full()
            .p(px(12.))
            .gap(px(2.))
            .bg(theme.card)
            .child(
                div()
                    .px(px(10.))
                    .pt(px(28.))
                    .pb(px(12.))
                    .text_size(TextSize::Sm.font_size())
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("beui for lathe"),
            )
            .children(Story::ALL.into_iter().map(|story| {
                let selected = story == self.story;
                let hover = theme.muted_hover();
                div()
                    .id(story.title())
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(beui::theme::radius::LG)
                    .cursor_pointer()
                    .text_size(TextSize::Sm.font_size())
                    .when(selected, |d| d.bg(theme.card_strong).font_weight(FontWeight::MEDIUM))
                    .when(!selected, |d| d.text_color(theme.muted_foreground).hover(move |s| s.bg(hover)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.story = story;
                        cx.notify();
                    }))
                    .child(story.title())
            }))
            .child(div().flex_1())
            .child(
                Button::new("appearance")
                    .icon(if dark { IconName::Sun } else { IconName::Moon })
                    .label(if dark { "Light" } else { "Dark" })
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(move |_, _, cx| {
                        let next = if dark { Appearance::Light } else { Appearance::Dark };
                        beui::theme::set_appearance(next, cx);
                    }),
            )
    }

    fn story(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.story {
            Story::AgentPanel => {
                let shown = self.replay.as_ref().map_or(SESSION_LEN, |r| r.shown);
                agent_panel(&self.panel_prompt, self.started, self.replays, shown, cx).into_any_element()
            }
            Story::Colors => colors(cx).into_any_element(),
            Story::Typography => typography().into_any_element(),
            Story::Icons => icons(cx).into_any_element(),
            Story::Spark => spark_story(cx).into_any_element(),
            Story::Buttons => buttons().into_any_element(),
            Story::Badges => badges().into_any_element(),
            Story::Messages => messages(self.started).into_any_element(),
            Story::Tools => tools().into_any_element(),
            Story::Diffs => diffs().into_any_element(),
            Story::Plan => narrow(TodoList::new("plan", beui_plan()).title("Implementation plan")).into_any_element(),
            Story::Hunks => hunks_story(&self.hunks, cx).into_any_element(),
            Story::Inline => inline_story(&self.inline, &self.inline_hunks, cx).into_any_element(),
            Story::Editor => editor_story(&self.editor.clone(), &self.lsp.status.clone(), cx).into_any_element(),
            Story::Select => select_story(self.choice, cx).into_any_element(),
            Story::Prompt => prompt_story(&self.prompt, self.notice.clone(), cx).into_any_element(),
        }
    }
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let panel = self.story == Story::AgentPanel;
        div()
            .flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(FONT_FAMILY)
            .child(self.sidebar(cx))
            .child(
                div()
                    .id("story")
                    .flex_1()
                    .h_full()
                    .when(!panel, |d| {
                        d.overflow_y_scroll().p(px(40.)).child(
                            div()
                                .pb(px(28.))
                                .text_size(TextSize::Xl.font_size())
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.story.title()),
                        )
                    })
                    .child(self.story(cx)),
            )
    }
}

/// A titled group of examples.
fn section(title: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .pb(px(32.))
        .child(div().text_size(TextSize::Xs.font_size()).font_weight(FontWeight::MEDIUM).opacity(0.6).child(title))
        .child(content)
}

fn row() -> gpui_kit::Div {
    div().flex().flex_wrap().items_center().gap(px(12.))
}

/// The width of the agent panel, so stories show components at the size they will have.
fn narrow(content: impl IntoElement) -> impl IntoElement {
    div().max_w(px(560.)).child(content)
}

fn colors(cx: &App) -> impl IntoElement {
    let t = cx.theme();
    let swatches: [(&str, Hsla); 19] = [
        ("background", t.background),
        ("card", t.card),
        ("card strong", t.card_strong),
        ("foreground", t.foreground),
        ("muted foreground", t.muted_foreground),
        ("primary", t.primary),
        ("primary foreground", t.primary_foreground),
        ("accent", t.accent),
        ("info", t.info),
        ("danger", t.danger),
        ("success", t.success),
        ("warning", t.warning),
        ("mark running", t.status_tone(StatusTone::Running)),
        ("mark done", t.status_tone(StatusTone::Done)),
        ("mark failed", t.status_tone(StatusTone::Failed)),
        ("mark pending", t.status_tone(StatusTone::Pending)),
        ("mark cancelled", t.status_tone(StatusTone::Cancelled)),
        ("diff added", t.diff_line(true)),
        ("diff removed", t.diff_line(false)),
    ];
    row().children(swatches.into_iter().map(move |(name, color)| {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w(px(120.))
            .child(div().h(px(64.)).rounded(beui::theme::radius::XL).bg(color))
            .child(div().text_size(TextSize::Xs.font_size()).child(name))
    }))
}

fn typography() -> impl IntoElement {
    let sizes = [("xl", TextSize::Xl), ("lg", TextSize::Lg), ("base", TextSize::Base), ("sm", TextSize::Sm), ("xs", TextSize::Xs)];
    let weights = [("Regular", FontWeight::NORMAL), ("Medium", FontWeight::MEDIUM), ("Semibold", FontWeight::SEMIBOLD)];
    div()
        .child(section(
            "Sizes",
            div().flex().flex_col().gap(px(8.)).children(sizes.into_iter().map(|(name, size)| {
                div()
                    .text_size(size.font_size())
                    .line_height(size.line_height())
                    .child(format!("{name}: Refactor the parser to stream tokens."))
            })),
        ))
        .child(section(
            "Weights",
            div().flex().flex_col().gap(px(8.)).children(weights.into_iter().map(|(name, weight)| {
                div().text_size(TextSize::Base.font_size()).font_weight(weight).child(format!("Geist {name}"))
            })),
        ))
        .child(section(
            "Mono",
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .font_family(MONO_FONT_FAMILY)
                .text_size(TextSize::Xs.font_size())
                .child("cargo test -p beui --release")
                .child("crates/beui/src/file_diff.rs  +42 -7")
                .child("0123456789 → ≠ ≤ ≥"),
        ))
}

fn icons(cx: &App) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    row().gap(px(20.)).children(IconName::ALL.iter().map(move |&name| {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .w(px(80.))
            .child(Icon::new(name).size(px(22.)))
            .child(div().text_size(px(11.)).text_color(muted).child(name.name()))
    }))
}

/// Every spark state at 20px, the largest size the app uses, with its name.
fn spark_story(cx: &App) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    row().gap(px(20.)).children(SparkState::ALL.iter().map(move |&state| {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .w(px(80.))
            .child(Spark::new(SharedString::from(format!("spark-{}", state.name())), state).size(px(20.)))
            .child(div().text_size(px(11.)).text_color(muted).child(state.name()))
    }))
}

fn buttons() -> impl IntoElement {
    let variants =
        [("Primary", ButtonVariant::Primary), ("Secondary", ButtonVariant::Secondary), ("Ghost", ButtonVariant::Ghost)];
    div()
        .child(section(
            "Variants",
            row().children(variants.into_iter().map(|(name, variant)| {
                Button::new(SharedString::from(format!("variant-{name}"))).label(name).variant(variant)
            })),
        ))
        .child(section(
            "Sizes",
            row()
                .child(Button::new("size-sm").label("Small"))
                .child(Button::new("size-md").label("Medium").size(ButtonSize::Md))
                .child(Button::new("size-lg").label("Large").size(ButtonSize::Lg)),
        ))
        .child(section(
            "With a chip: hover to slide the arrow",
            row()
                .child(Button::new("chip-sm").label("New session").chip(IconName::ArrowRight))
                .child(Button::new("chip-md").label("Continue").chip(IconName::ArrowRight).size(ButtonSize::Md))
                .child(Button::new("chip-lg").label("Get started").chip(IconName::ArrowRight).size(ButtonSize::Lg)),
        ))
        .child(section(
            "Icon buttons",
            row()
                .child(Button::new("i-send").icon(IconName::ArrowUp).size(ButtonSize::Icon))
                .child(Button::new("i-stop").icon(IconName::Square).size(ButtonSize::Icon).variant(ButtonVariant::Secondary))
                .child(Button::new("i-attach").icon(IconName::Paperclip).size(ButtonSize::Icon).variant(ButtonVariant::Ghost))
                .child(Button::new("i-more").icon(IconName::Ellipsis).size(ButtonSize::Icon).variant(ButtonVariant::Ghost)),
        ))
        .child(section("Disabled", row().child(Button::new("disabled").label("Send").disabled(true))))
}

fn badges() -> impl IntoElement {
    div()
        .child(section(
            "Tones",
            row()
                .child(Badge::new("Sonnet 5"))
                .child(Badge::new("Plan mode").tone(Tone::Primary))
                .child(Badge::new("Passed").tone(Tone::Success))
                .child(Badge::new("Needs approval").tone(Tone::Warning))
                .child(Badge::new("Failed").tone(Tone::Danger)),
        ))
        .child(section("Keys", row().child(Kbd::new("⌘↵")).child(Kbd::new("⌘K")).child(Kbd::new("Esc")).child(Kbd::new("⇧Tab"))))
        .child(section("Spinner", row().child(Spinner::new("spin-a")).child(Spinner::new("spin-b").size(px(20.)))))
}

const REPLY: &str = "I found the bug. `parse_hunk` counts the header line as a context line, so every \
number after it is off by one.\n\nI will:\n\n1. Skip the `@@` line when counting.\n2. Add a test for a hunk that starts at line 1.\n\n\
The fix is in `crates/beui/src/file_diff.rs`.";

const SUMMARY: &str = "The release is ready for a focused rollout. The main conversation flow, keyboard \
navigation, error recovery, and reduced-motion behavior are all covered.\n\nI would keep advanced workflow \
controls out of this version. They add configuration without improving the first-run experience, and the \
usage data from this release will give us a better basis for those decisions.\n\nBefore publishing, run the \
accessibility suite once more and verify the streaming behavior with a long response on a smaller viewport.";

/// Thinking since `ago` before `started`, so a row can show a later label.
fn thinking_for(started: Instant, ago: u64) -> ThinkingPhase {
    ThinkingPhase::Thinking { since: started.checked_sub(Duration::from_secs(ago)).unwrap_or(started) }
}

/// Every status row the agent shows: each shimmer, the breath, the requesting glimmer, a later label,
/// and a finished thought.
fn working(started: Instant) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .child(Thinking::new("think", ThinkingPhase::Thinking { since: started }).elapsed("4s").tokens(1_234))
        .child(
            Thinking::new("think-stepped", ThinkingPhase::Thinking { since: started })
                .style(ThinkingStyle::Shimmer(Shimmer::Stepped))
                .elapsed("4s"),
        )
        .child(Thinking::new("think-breath", thinking_for(started, 16)).style(ThinkingStyle::Breath).elapsed("16s"))
        .child(Thinking::new("sending", ThinkingPhase::Sending))
        .child(Thinking::new("tools", ThinkingPhase::RunningTools).elapsed("31s").tasks(3))
        .child(Thinking::new("orbit", ThinkingPhase::Thinking { since: started }).elapsed("9s").subagents(2))
        .child(Thinking::new("thought", ThinkingPhase::Thought { seconds: 4 }))
}

fn messages(started: Instant) -> impl IntoElement {
    narrow(
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(section("Working", working(started)))
            .child(section(
                "Tones",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(MessageBubble::text("tone-solid", "Solid: the user's own turn.").variant(MessageBubbleVariant::Solid).align(MessageBubbleAlign::End))
                    .child(MessageBubble::text("tone-soft", "Soft: the default reply.").variant(MessageBubbleVariant::Soft))
                    .child(MessageBubble::text("tone-tint", "Tint: reads the same as soft here.").variant(MessageBubbleVariant::Tint))
                    .child(MessageBubble::text("tone-borderless", "Borderless: a card_strong fill stands in for beui's outline border.").variant(MessageBubbleVariant::Borderless))
                    .child(MessageBubble::text("tone-danger", "Danger: the tool call failed.").variant(MessageBubbleVariant::Danger))
                    .child(MessageBubble::text("tone-ghost", "Ghost: no fill, stretches to the full row.").variant(MessageBubbleVariant::Ghost)),
            ))
            .child(section(
                "Grouped",
                message_bubble_group(MessageBubbleGroupSpacing::Compact)
                    .child(MessageBubble::text("group-a", "Can you turn these notes into a launch update?").variant(MessageBubbleVariant::Solid).align(MessageBubbleAlign::End))
                    .child(MessageBubble::text("group-b", "Absolutely. I'll keep it concise and lead with what changed.").align(MessageBubbleAlign::Start))
                    .child(MessageBubble::text("group-c", "Do you want the tone more technical or more customer-facing?").align(MessageBubbleAlign::Start)),
            ))
            .child(section(
                "Expandable",
                MessageBubble::new(
                    "collapsible",
                    MessageBubbleCollapsible::new(
                        "collapsible-body",
                        div().flex().flex_col().gap(px(8.)).children(SUMMARY.split("\n\n").map(|p| div().child(p.to_string()))),
                    )
                    .collapsed_lines(4),
                ),
            ))
            .child(section(
                "Streaming",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(AgentText::new("stream-live", "Absolutely. I'll keep it conc").status(AgentTextStatus::Streaming))
                    .child(
                        AgentText::new("stream-done", REPLY)
                            .status(AgentTextStatus::Complete)
                            .copy_text(REPLY)
                            .sources(vec![
                                AgentTextSource::new("Motion for React", "motion.dev"),
                                AgentTextSource::new("ARIA live regions", "developer.mozilla.org"),
                            ]),
                    )
                    .child(
                        AgentText::new("stream-error", "The request timed out before a full answer arrived.")
                            .status(AgentTextStatus::Error)
                            .copy_text("The request timed out before a full answer arrived."),
                    ),
            ))
    )
}

const TEST_OUTPUT: &str = "running 23 tests\ntest file_diff::tests::stats_count_added_and_removed_lines ... ok\n\
test file_diff::tests::lines_are_numbered_from_the_hunk_start ... ok\ntest todo_list::tests::progress_counts_only_done_steps ... ok\n\
test tool_call::tests::short_output_is_kept_whole ... ok\ntest tool_call::tests::long_output_keeps_the_first_lines ... ok\n\
test theme::tests::mixing_from_clear_keeps_the_target_color ... ok\ntest motion::tests::springs_settle ... ok\n\
test motion::tests::reduce_motion_jumps ... ok\ntest theme::tests::the_system_appearance_picks_light_or_dark ... ok\n\
test file_diff::tests::the_sign_is_not_part_of_the_text ... ok\ntest file_diff::tests::file_headers_are_dropped ... ok\n\
test todo_list::tests::an_empty_plan_has_no_progress ... ok\ntest theme::tests::added_and_removed_lines ... ok\n\
test result: ok. 23 passed; 0 failed";

fn tools() -> impl IntoElement {
    narrow(
        div()
            .flex()
            .flex_col()
            .child(section(
                "States",
                div()
                    .flex()
                    .flex_col()
                    .child(ToolCall::new("t-read", "Read file").tool("crates/beui/src/file_diff.rs").meta("214 lines").status(ToolStatus::Done))
                    .child(ToolCall::new("t-grep", "Searched code").tool("fn hunk_starts").status(ToolStatus::Done).output("crates/beui/src/file_diff.rs:69: fn hunk_starts(header: &str) -> (u32, u32) {"))
                    .child(ToolCall::new("t-test", "Ran tests").tool("cargo test -p beui").meta("3.1s").status(ToolStatus::Done).output(TEST_OUTPUT))
                    .child(ToolCall::new("t-run", "Running clippy").tool("cargo clippy --workspace").status(ToolStatus::Running).output("Checking beui v0.1.0\n    Checking beui-gallery v0.1.0"))
                    .child(ToolCall::new("t-fail", "Fetched theme").tool("https://beui.dev/docs/theme").status(ToolStatus::Failed).output("error: request timed out after 30s"))
                    .child(ToolCall::new("t-wait", "Push to main").tool("git push origin main").status(ToolStatus::Cancelled)),
            ))
            .child(section(
                "Approval",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        ToolApproval::new("approve-pending", "terminal.run")
                            .description("The agent wants to run the project test suite in the current workspace.")
                            .parameter_code("Command", "bun test tests/a11y.test.tsx")
                            .parameter("Directory", "ui-components")
                            .default_open(true)
                            .on_approve(|_, _, _| {})
                            .on_always_allow(|_, _, _| {})
                            .on_deny(|_, _, _| {}),
                    )
                    .child(
                        ToolApproval::new("approve-running", "terminal.run")
                            .title("Terminal access")
                            .description("The agent wants to run the project test suite in the current workspace.")
                            .parameter_code("Command", "bun test tests/a11y.test.tsx")
                            .parameter("Directory", "ui-components")
                            .status(ToolApprovalStatus::Running),
                    )
                    .child(
                        ToolApproval::new("approve-complete", "terminal.run")
                            .title("Terminal access")
                            .description("The agent wants to run the project test suite in the current workspace.")
                            .parameter_code("Command", "bun test tests/a11y.test.tsx")
                            .parameter("Directory", "ui-components")
                            .status(ToolApprovalStatus::Complete),
                    )
                    .child(
                        ToolApproval::new("approve-denied", "terminal.run")
                            .title("Terminal access")
                            .description("The agent wants to run the project test suite in the current workspace.")
                            .status(ToolApprovalStatus::Denied),
                    )
                    .child(
                        ToolApproval::new("approve-error", "terminal.run")
                            .title("Terminal access")
                            .description("The test suite could not start.")
                            .status(ToolApprovalStatus::Error),
                    ),
            )),
    )
}

const DIFF: &str = "\
--- a/crates/beui/src/file_diff.rs
+++ b/crates/beui/src/file_diff.rs
@@ -66,7 +66,7 @@ impl DiffLine {
 /// The line before each side's first line.
 fn hunk_starts(header: &str) -> (u32, u32) {
     let start = |sign: char| {
-            .map_or(0, |n| n)
+            .map_or(0, |n| n.saturating_sub(1)) // The header names the first line, so the line before it is one less, and a long comment scrolls sideways.
     };
     (start('-'), start('+'))
 }";

const CODE: &str = "#[test]\nfn a_hunk_at_line_one_starts_at_one() {\n    let lines = DiffLine::parse(\"@@ -1,1 +1,1 @@\\n a\");\n    assert_eq!(lines[1].number, Some(1));\n}";

fn diffs() -> impl IntoElement {
    // Mirrors beui's file-diff.preview.tsx (a settled diff with its Copy button, and a still-streaming
    // one) and code-block.preview.tsx (a highlighted, completed block, and a still-streaming one).
    let diff_a = DiffLine::parse(DIFF);
    let diff_a_copy = diff_a.iter().map(|l| l.text.to_string()).collect::<Vec<_>>().join("\n");
    narrow(
        div()
            .flex()
            .flex_col()
            .child(section(
                "File diff",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        FileDiff::new("diff-a", "crates/beui/src/file_diff.rs", diff_a)
                            .default_open(true)
                            .status(FileDiffStatus::Complete)
                            .copy_text(diff_a_copy),
                    )
                    .child(
                        FileDiff::new(
                            "diff-b",
                            "crates/beui/src/file_diff/tests.rs",
                            DiffLine::parse("@@ -40,0 +41,6 @@\n+#[test]\n+fn one() {}\n"),
                        )
                        .status(FileDiffStatus::Streaming)
                        .max_height(150.),
                    ),
            ))
            .child(section(
                "Code block",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        CodeBlock::new("code", CODE)
                            .title("file_diff/tests.rs")
                            .language("rust")
                            .highlight_lines([3, 4])
                            .max_height(224.),
                    )
                    .child(
                        CodeBlock::new("code-streaming", "pub fn run(task: Task) -> Result<()> {\n    execute(task)")
                            .title("runner.rs")
                            .language("rust")
                            .status(CodeBlockStatus::Streaming)
                            .max_height(150.),
                    ),
            )),
    )
}

/// The plan from beui's own todo-list preview, to compare against it.
fn beui_plan() -> Vec<Todo> {
    vec![
        Todo::new("data-flow", "Inspect the current data flow", TodoStatus::Done),
        Todo::new("schema", "Update the response schema", TodoStatus::Done),
        Todo::new("edge-cases", "Add coverage for edge cases", TodoStatus::Done),
        Todo::new("checks", "Run checks and prepare the result", TodoStatus::InProgress).detail("100%"),
    ]
}

/// beui's `PromptInputPreview`: the box centered in a fixed `h-[360px] max-w-xl`, with the sent-prompt
/// or picked-action notice on its own `h-8` line underneath.
fn prompt_story(prompt: &Entity<PromptInput>, notice: Option<SharedString>, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .flex()
        .h(px(360.))
        .w_full()
        .max_w(px(576.))
        .flex_col()
        .justify_center()
        .child(prompt.clone())
        .child(
            div()
                .h(px(32.))
                .px(px(8.))
                .pt(px(8.))
                .text_size(TextSize::Xs.font_size())
                .text_color(theme.muted_foreground)
                .children(notice),
        )
}

const SAMPLE_RUST: &str = r#"//! The line numbers a hunk starts at.
//!
//! `width` below is called by `broken`, which also holds a real type error for the server to find.

pub fn width() -> u32 {
    7
}

pub fn broken() -> u32 {
    let text: u32 = "not a number";
    text + width()
}

use std::collections::HashMap;

/// Parses `@@ -12,4 +12,5 @@` into the line before each side's first line.
fn hunk_starts(header: &str) -> (u32, u32) {
    let start = |sign: char| {
        header
            .split_whitespace()
            .find_map(|part| part.strip_prefix(sign))
            .and_then(|range| range.split(',').next()?.parse::<u32>().ok())
            .map_or(0, |n| n.saturating_sub(1))
    };
    (start('-'), start('+'))
}

#[derive(Debug, Default)]
struct Counts {
    added: usize,
    removed: usize,
    by_file: HashMap<String, usize>,
}

impl Counts {
    fn total(&self) -> usize {
        self.added + self.removed
    }
}
"#;

/// The Editor story's language server.
struct LspState {
    /// The server, once it has started. `None` means it is starting, not installed, or failed.
    worker: Option<lathe_lsp::LspWorker>,
    /// What the status line says: the server's name, an error, or what it last answered.
    status: SharedString,
    /// The one question in flight from a button or F12. A newer one replaces it.
    pending: Task<()>,
    /// The diagnostics request in flight. A newer one replaces it.
    checking: Task<()>,
    /// The timer that re-checks once typing pauses.
    recheck: Task<()>,
    /// Watches the buffer for edits, once the server is ready.
    edits: Option<gpui_kit::Subscription>,
}

/// How long typing must pause before the buffer is checked again. docs/code-editor.md sets 150ms.
const RECHECK_AFTER: Duration = Duration::from_millis(150);

impl Default for LspState {
    fn default() -> Self {
        Self {
            worker: None,
            status: SharedString::default(),
            pending: Task::ready(()),
            checking: Task::ready(()),
            recheck: Task::ready(()),
            edits: None,
        }
    }
}

/// The key a Zed user holds to follow a symbol.
const SECONDARY: &str = if cfg!(target_os = "macos") { "⌘" } else { "ctrl" };

/// The editable file. Typing, selection, undo and search all come from gpui-base; the skin is ours.
/// Its diagnostics and its definitions come from a real rust-analyzer when one is installed.
/// Both sides of two hunks, as real text in one buffer. Rows 1 and 8 are the old code; rows 2 to 3
/// and row 9 are the agent's. Nothing is virtual, which is why the buffer stays writable.
const INLINE_FILE: &str = "pub struct Config {\n    pub width: u32,\n    pub width: u32,\n    pub height: u32,\n}\n\nimpl Config {\n    pub fn new() -> Self {\n        Self { width: 80 }\n        Self { width: 80, height: 24 }\n    }\n}\n";

fn inline_fixture() -> Vec<InlineHunk> {
    vec![InlineHunk::new("field", 1..2, 2..4), InlineHunk::new("ctor", 8..9, 9..10)]
}

fn inline_story(
    state: &Entity<gpui_kit::component::input::EditorState>,
    hunks: &[InlineHunk],
    cx: &mut Context<Gallery>,
) -> impl IntoElement {
    let left = beui::inline_review::pending_count(hunks, &[]);
    let decide = cx.listener(
        |this: &mut Gallery, (id, decision): &(SharedString, Decision), window: &mut Window, cx| {
            let Some(hunk) = this.inline_hunks.iter().find(|h| &h.id == id).cloned() else { return };
            let closed = hunk.closing(*decision);
            beui::inline_review::apply(&this.inline, &[(hunk, *decision)], window, cx);
            this.inline_hunks = beui::inline_review::shift_after(&this.inline_hunks, id, &closed);
            // The decision already moved the hunks; the edit it made must not move them again.
            this.inline_text = this.inline.read(cx).value().to_string();
            cx.notify();
        },
    );
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .max_w(px(760.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(TextSize::Xs.font_size())
                .child(div().font_family(MONO_FONT_FAMILY).child("src/config.rs"))
                .child(Badge::new(format!("{left} left")))
                .child(div().flex_1())
                .child(div().text_color(cx.theme().muted_foreground).child("type anywhere; it stays writable")),
        )
        .child(
            InlineReview::new("inline", state, hunks.to_vec())
                .height(px(260.))
                // The first hunk left is the current one, so its bar stays up without a pointer. That
                // is also the only way a headless screenshot can show one.
                .when_some(hunks.first(), |review, hunk| review.current(hunk.id.clone()))
                .on_decide(move |id, decision, window, cx| decide(&(id.clone(), decision), window, cx)),
        )
}

fn editor_story(
    state: &Entity<gpui_kit::component::input::EditorState>,
    lsp_status: &SharedString,
    cx: &mut Context<Gallery>,
) -> impl IntoElement {
    let go = cx.listener(|this, _, window, cx| this.go_to_definition(window, cx));
    let check = cx.listener(|this, _, _, cx| this.pull_diagnostics(true, cx));
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .max_w(px(760.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(TextSize::Xs.font_size())
                .child(div().font_family(MONO_FONT_FAMILY).child("src/lib.rs"))
                .child(Badge::new("rust"))
                .child(div().flex_1())
                .child(Button::new("lsp-check").label("Check").size(ButtonSize::Chip).on_click(check))
                .child(
                    Button::new("lsp-go")
                        .label("Go to definition")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Chip)
                        .on_click(go),
                ),
        )
        .child(
            div()
                .on_action(cx.listener(|this, _: &GoToDefinition, window, cx| this.go_to_definition(window, cx)))
                .child(CodeEditor::new(state).height(px(380.))),
        )
        .child(
            div()
                .text_size(TextSize::Xs.font_size())
                .text_color(cx.theme().muted_foreground)
                .child(lsp_status.clone()),
        )
}

/// The agent's edit to one file, as the review shows it. Accept and Reject resolve for real, so the
/// rows close and the survivors settle into plain code.
fn sample_hunks() -> Vec<Hunk> {
    let row = |kind: DiffLineKind, old: Option<u32>, new: Option<u32>, text: &str| DiffLine {
        kind,
        old_line: old,
        new_line: new,
        text: text.to_string().into(),
    };
    vec![
        Hunk::new(
            "h1",
            vec![
                row(DiffLineKind::Context, Some(64), Some(64), "/// The line before each side's first line."),
                row(DiffLineKind::Context, Some(65), Some(65), "fn hunk_starts(header: &str) -> (u32, u32) {"),
                row(DiffLineKind::Context, Some(66), Some(66), "    let start = |sign: char| {"),
                row(DiffLineKind::Removed, Some(67), None, "            .map_or(0, |n| n)"),
                row(DiffLineKind::Added, None, Some(67), "            .map_or(0, |n| n.saturating_sub(1))"),
                row(DiffLineKind::Context, Some(68), Some(68), "    };"),
            ],
        ),
        Hunk::new(
            "h2",
            vec![
                row(DiffLineKind::Context, Some(96), Some(96), "#[test]"),
                row(DiffLineKind::Added, None, Some(97), "fn a_hunk_at_line_one_starts_at_one() {"),
                row(DiffLineKind::Added, None, Some(98), "    let lines = DiffLine::parse(\"@@ -1,1 +1,1 @@\\n a\");"),
                row(DiffLineKind::Added, None, Some(99), "    assert_eq!(lines[1].new_line, Some(1));"),
                row(DiffLineKind::Added, None, Some(100), "}"),
                row(DiffLineKind::Context, Some(97), Some(101), ""),
            ],
        ),
    ]
}

fn hunks_story(hunks: &[Hunk], cx: &mut Context<Gallery>) -> impl IntoElement {
    let resolve = |state: HunkState, cx: &mut Context<Gallery>| {
        let gallery = cx.entity().downgrade();
        move |id: &SharedString, _: &mut Window, cx: &mut App| {
            let id = id.clone();
            gallery
                .update(cx, |g, cx| {
                    if let Some(hunk) = g.hunks.iter_mut().find(|h| h.id == id) {
                        hunk.state = state;
                    }
                    cx.notify();
                })
                .ok();
        }
    };
    let reset = cx.listener(|this, _, _, cx| {
        this.hunks = sample_hunks();
        cx.notify();
    });
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .max_w(px(720.))
        .child(
            HunkReview::new("review", "crates/beui/src/file_diff.rs", hunks.to_vec())
                .on_accept(resolve(HunkState::Accepted, cx))
                .on_reject(resolve(HunkState::Rejected, cx))
                .on_accept_all(cx.listener(|this, _, _, cx| {
                    for hunk in &mut this.hunks {
                        hunk.state = HunkState::Accepted;
                    }
                    cx.notify();
                }))
                .on_reject_all(cx.listener(|this, _, _, cx| {
                    for hunk in &mut this.hunks {
                        hunk.state = HunkState::Rejected;
                    }
                    cx.notify();
                })),
        )
        .child(
            div().child(
                Button::new("hunks-reset").label("Reset the file").variant(ButtonVariant::Ghost).on_click(reset),
            ),
        )
}

fn select_story(choice: Option<usize>, cx: &mut Context<Gallery>) -> impl IntoElement {
    let gallery = cx.entity().downgrade();
    div().w(px(260.)).child(
        Select::new("framework", ["Next.js", "Remix", "Astro", "SvelteKit", "Nuxt"])
            .placeholder("Pick a framework")
            .default_open(std::env::var("GALLERY_OPEN").is_ok())
            .selected(choice)
            .on_change(move |i, _, cx| {
                gallery.update(cx, |g, cx| {
                    g.choice = Some(i);
                    cx.notify();
                }).ok();
            }),
    )
}

fn sample_plan() -> Vec<Todo> {
    vec![
        Todo::new("find", "Find where hunk numbers start", TodoStatus::Done),
        Todo::new("fix", "Fix the off-by-one in hunk_starts", TodoStatus::Done),
        Todo::new("test-line-1", "Add a test for a hunk at line 1", TodoStatus::InProgress),
        Todo::new("suite", "Run the full test suite", TodoStatus::Pending),
    ]
}

/// How many items the Agent panel's session holds.
const SESSION_LEN: usize = 9;

/// The session's first `shown` chat items, as the panel lists them. The two reads stack tight in their
/// own list, so each still enters on its own.
fn session_list(started: Instant, replay: usize, shown: usize) -> EntranceList {
    let tools = [
        ("s-read", ToolCall::new("s-read", "Read file").tool("crates/beui/src/file_diff.rs").meta("214 lines").status(ToolStatus::Done)),
        ("s-grep", ToolCall::new("s-grep", "Searched code").tool("fn hunk_starts").status(ToolStatus::Done)),
    ];
    let tools = tools.into_iter().take(shown.saturating_sub(1)).fold(
        EntranceList::new(ElementId::NamedInteger("session-tools".into(), replay as u64), div().flex().flex_col()),
        |list, (id, item)| list.item(id, item),
    );
    let items: [(&'static str, AnyElement); SESSION_LEN - 1] = [
        ("s-user", MessageBubble::text("s-user", "The line numbers in the diff view are off by one. Can you fix it?").variant(MessageBubbleVariant::Solid).align(MessageBubbleAlign::End).into_any_element()),
        ("s-tools", tools.into_any_element()),
        ("s-reply", AgentText::new("s-reply", REPLY).status(AgentTextStatus::Complete).copy_text(REPLY).into_any_element()),
        ("s-plan", TodoList::new("s-plan", sample_plan()).into_any_element()),
        ("s-diff", FileDiff::new("s-diff", "crates/beui/src/file_diff.rs", DiffLine::parse(DIFF)).status(FileDiffStatus::Complete).into_any_element()),
        ("s-test", ToolCall::new("s-test", "Ran tests").tool("cargo test -p beui").meta("3.1s").status(ToolStatus::Done).output(TEST_OUTPUT).into_any_element()),
        (
            "s-push",
            ToolApproval::new("s-push", "git push origin main")
                .description("Push the fix so CI can run the full test suite.")
                .parameter("Directory", "~/Documents/lathe")
                .default_open(true)
                .on_approve(|_, _, _| {})
                .on_always_allow(|_, _, _| {})
                .on_deny(|_, _, _| {})
                .into_any_element(),
        ),
        ("s-think", Thinking::new("s-think", thinking_for(started, 18)).elapsed("18s").tokens(3_400).subagents(2).into_any_element()),
    ];
    // The two reads share one row of this list, so once both show it holds one item fewer.
    let rows = if shown >= 3 { shown - 1 } else { shown };
    items.into_iter().take(rows).fold(
        EntranceList::new(ElementId::NamedInteger("session-list".into(), replay as u64), div().flex().flex_col().gap(px(16.))),
        |list, (id, item)| list.item(id, item),
    )
}

/// A whole session as the panel will show it, at the panel's width. `replay` names the list, so each
/// replay starts a fresh one; `shown` is how many items it holds so far.
fn agent_panel(prompt: &Entity<PromptInput>, started: Instant, replay: usize, shown: usize, cx: &mut Context<Gallery>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let header = pane_header("Claude Code", cx)
        .child(Badge::new("Sonnet 5"))
        .child(div().flex_1())
        .child(
            Button::new("p-replay")
                .label("Replay session")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(|this, _, _, cx| this.start_replay(cx))),
        )
        .child(Button::new("p-new").icon(IconName::Plus).variant(ButtonVariant::Ghost).size(ButtonSize::Icon))
        .child(Button::new("p-more").icon(IconName::Ellipsis).variant(ButtonVariant::Ghost).size(ButtonSize::Icon));

    let list = session_list(started, replay, shown);
    let session = div().id("session").flex_1().overflow_y_scroll().px(px(20.)).py(px(20.)).child(list);

    // One row per subagent, above the composer. They arrive with the rest of the replay.
    let subagents = [
        SubagentRow::new("sa-explore", "Explore", "Find every caller of hunk_starts").elapsed("12s"),
        SubagentRow::new("sa-test", "Test runner", "Run the diff parser tests").elapsed("5s"),
        SubagentRow::new("sa-review", "Review", "Check the off-by-one fix").finished(Some(38)),
    ];
    let agents = subagents.into_iter().enumerate().filter(|(i, _)| shown >= SESSION_LEN.saturating_sub(3) + i).fold(
        EntranceList::new(ElementId::NamedInteger("subagents".into(), replay as u64), div().flex().flex_col().gap(px(4.))),
        |list, (i, row)| list.item(("subagent", i), row),
    );

    div()
        .flex()
        .justify_center()
        .size_full()
        .bg(theme.card)
        .child(
            div()
                .flex()
                .flex_col()
                .w(px(560.))
                .h_full()
                .bg(theme.background)
                .child(header)
                .child(session)
                .child(div().flex().flex_col().gap(px(8.)).p(px(12.)).child(agents).child(prompt.clone())),
        )
}

gpui_kit::actions!(gallery, [
    /// F12: jump from the caret to where the symbol under it is defined.
    GoToDefinition
]);

fn main() {
    gpui_kit::application().with_assets(beui::Assets).run(|cx| {
        beui::init(cx);
        cx.bind_keys([gpui_kit::KeyBinding::new("f12", GoToDefinition, Some("Input"))]);
        match std::env::var("GALLERY_THEME").as_deref() {
            Ok("dark") => beui::theme::set_appearance(Appearance::Dark, cx),
            Ok("light") => beui::theme::set_appearance(Appearance::Light, cx),
            _ => {}
        }
        let bounds = Bounds::centered(None, size(px(1100.), px(860.)), cx);
        cx.open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            |window, cx| {
                let gallery = cx.new(|cx| Gallery::new(window, cx));
                // gpui-component inputs need its Root at the top of the window.
                cx.new(|cx| gpui_kit::component::Root::new(gallery, window, cx))
            },
        )
        .expect("open the gallery window");
        cx.activate(true);
    });
}
