//! Shows every atelier-ui component in each state, in light and dark. Run with `cargo run -p atelier-gallery`.
//! `GALLERY_STORY=<title>` opens a story and `GALLERY_THEME=<theme name>` (or `light`, `dark`) picks the theme, so a script can
//! screenshot any of them. Without it, the theme last picked in the sidebar comes back (`settings`).
//! `GALLERY_REPLAY=1` starts the Agent panel's "Replay session" on launch, so a capture can see the entrances.

use atelier_ui::{
    ActiveTheme, AgentText, AgentTextSource, AgentTextStatus, Appearance, Badge, Button, ButtonSize, ButtonVariant,
    CodeBlock, CodeBlockStatus, DiffLine, FONT_FAMILY, FileDiff, FileDiffStatus, Icon, IconName, Kbd, MONO_FONT_FAMILY,
    MessageBubble, MessageBubbleAlign, MessageBubbleCollapsible, MessageBubbleGroupSpacing, MessageBubbleVariant,
    PromptAction, PromptInput, PromptInputEvent, PromptModel, Select, Shimmer, Spinner, TextSize, Thinking,
    ThinkingPhase, ThinkingStyle, Todo, TodoList,
    CodeEditor, Decision, InlineHunk, InlineReview, StatusTone, TodoStatus, ToolApproval, ToolApprovalStatus, ToolCall, ToolStatus, Tone, message_bubble_group,
};
use atelier_agents::claude::{self, SparkState};
use atelier_ui::context_usage::ContextPart;
use std::time::{Duration, Instant};


mod agent_panel;
mod agent_parts;
mod editor_story;
mod workers;
mod load_story;
mod panels_story;
mod sidebar_story;
mod pr_view_story;
mod motion_story;
mod tasks_story;
mod streaming_story;
mod voice_story;
mod variants_story;
mod replay_story;
mod merge_story;
mod pr_fixture;
mod pr_story;
mod review_story;
mod providers_story;
mod sign_in_story;

use gpui_kit::base::input::InputEvent;

use gpui_kit::{
    AnyElement, App, AppContext, Bounds, Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds, WindowOptions, div,
    Task, prelude::FluentBuilder, px, size,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Story {
    AgentPanel,
    ChangedFiles,
    Worktrees,
    Providers,
    ProviderSettings,
    SignInNotice,
    SubagentCard,
    SubagentStrip,
    PrCard,
    PrChip,
    ModelBadge,
    Review,
    PullRequest,
    PullRequests,
    Merge,
    Colors,
    Typography,
    Icons,
    Spark,
    Voice,
    Buttons,
    Badges,
    Messages,
    Tools,
    Diffs,
    Plan,
    Inline,
    Editor,
    Select,
    Prompt,
    Load,
    AgentSidebar,
    AgentPanels,
    AgentReplay,
    Tasks,
    Variants,
    Streaming,
    Motion,
    PullRequestView,
}

impl Story {
    const ALL: [Story; 39] = [
        Story::AgentPanel,
        Story::ChangedFiles,
        Story::Worktrees,
        Story::Providers,
        Story::ProviderSettings,
        Story::SignInNotice,
        Story::SubagentCard,
        Story::SubagentStrip,
        Story::PrCard,
        Story::PrChip,
        Story::ModelBadge,
        Story::Review,
        Story::PullRequest,
        Story::PullRequests,
        Story::Merge,
        Story::Colors,
        Story::Typography,
        Story::Icons,
        Story::Spark,
        Story::Voice,
        Story::Buttons,
        Story::Badges,
        Story::Messages,
        Story::Tools,
        Story::Diffs,
        Story::Plan,
        Story::Inline,
        Story::Editor,
        Story::Select,
        Story::Prompt,
        Story::Load,
        Story::AgentSidebar,
        Story::AgentPanels,
        Story::AgentReplay,
        Story::Tasks,
        Story::Variants,
        Story::Streaming,
        Story::PullRequestView,
        Story::Motion,
    ];

    fn title(self) -> &'static str {
        match self {
            Story::AgentPanel => "Agent panel",
            Story::ChangedFiles => "Changed files",
            Story::Worktrees => "Worktrees",
            Story::Providers => "Providers",
            Story::ProviderSettings => "Provider settings",
            Story::SignInNotice => "Sign-in notice",
            Story::SubagentCard => "Subagent card",
            Story::SubagentStrip => "Subagent strip",
            Story::PrCard => "PR card",
            Story::PrChip => "PR chip",
            Story::ModelBadge => "Model badge",
            Story::Review => "Review",
            Story::PullRequest => "Pull request",
            Story::PullRequests => "Pull requests",
            Story::Merge => "Merge",
            Story::Colors => "Colors",
            Story::Typography => "Typography",
            Story::Icons => "Icons",
            Story::Spark => "Spark",
            Story::Voice => "Voice",
            Story::Buttons => "Buttons",
            Story::Badges => "Badges and keys",
            Story::Messages => "Messages",
            Story::Tools => "Tool calls",
            Story::Diffs => "Diffs and code",
            Story::Plan => "Plan",
            Story::Inline => "Inline review",
            Story::Editor => "Editor",
            Story::Select => "Select",
            Story::Prompt => "Prompt input",
            Story::Load => "Highlight load",
            Story::AgentSidebar => "Sidebar",
            Story::AgentPanels => "Panels",
            Story::AgentReplay => "Agent replay",
            Story::PullRequestView => "Pull request view",
            Story::Tasks => "Tasks",
            Story::Variants => "Variants",
            Story::Streaming => "Streaming",
            Story::Motion => "Motion",
        }
    }
}

/// beui's preview models: a provider mark per model. The real favicons preview.tsx fetches over the
/// network have no equivalent here, so every model shows the generic `Bot` mark instead.
/// What fills the preview's context, as an agent that can break it down would tell it.
fn preview_context_parts() -> Vec<ContextPart> {
    [
        ("System prompt", 4_100),
        ("Tool definitions", 7_500),
        ("Skills", 11_100),
        ("MCP & dynamic tools", 5_900),
        ("Subagent definitions", 2_500),
        ("Summarized conversation", 5_500),
        ("Conversation", 69_700),
    ]
    .into_iter()
    .map(|(label, tokens)| ContextPart::new(label, tokens))
    .collect()
}

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
            .icon(IconName::AddPhoto),
        PromptAction::new("skill", "Use a skill")
            .description("Give the agent a specialized workflow.")
            .icon(IconName::Extension),
        PromptAction::new("context", "Add context")
            .description("Include a file with supporting details.")
            .icon(IconName::Description),
    ]
}

struct Gallery {
    /// Where Tab starts: with nothing focused, the window's Tab action has no path to walk from.
    focus: gpui_kit::FocusHandle,
    story: Story,
    choice: Option<usize>,
    /// The Inline review story's buffer: both sides of every hunk, as real text.
    inline: Entity<gpui_kit::component::input::EditorState>,
    /// The hunks still waiting in that buffer.
    inline_hunks: Vec<InlineHunk>,
    /// Hunks the user decided, fading before their edit runs.
    inline_resolving: Vec<atelier_ui::Resolve>,
    /// That buffer's text as the hunks last matched it, so an edit can move them with the rows.
    inline_text: String,
    /// The review before and after each decision, so undo and redo bring its hunks back too.
    inline_history: atelier_ui::inline_review::DecisionHistory,
    /// Moves the hunks whenever the user edits that buffer.
    _inline_edits: gpui_kit::Subscription,
    /// The Editor story's tabs: a real file per language, each with its language server.
    editors: editor_story::EditorTabs,
    /// The Review story, which keeps its own files, hunks and comments.
    review: Entity<review_story::ReviewStory>,
    /// The Pull request story: its rail, its bar and its diff.
    pull_request: Entity<pr_story::PrStory>,
    /// The Merge story, which keeps the reader's last method.
    merge: Entity<merge_story::MergeStory>,
    /// Built the first time it shows: it holds a 10k-line file.
    load: Option<Entity<load_story::LoadStory>>,
    agent_sidebar: Option<Entity<sidebar_story::SidebarStory>>,
    agent_panels: Option<Entity<panels_story::PanelsStory>>,
    agent_replay: Option<Entity<replay_story::ReplayStory>>,
    voice: Option<Entity<voice_story::VoiceStory>>,
    pr_view: Option<Entity<pr_view_story::PrViewStory>>,
    tasks: Option<Entity<tasks_story::TasksStory>>,
    variants: Option<Entity<variants_story::VariantsStory>>,
    streaming: Option<Entity<streaming_story::StreamingStory>>,
    motion: Option<Entity<motion_story::MotionStory>>,
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
    /// The live clock for the agent parts: see [`agent_parts`].
    tick: usize,
    /// Advances `tick` while the stories play live.
    live: Option<Task<()>>,
    _system: [gpui_kit::Subscription; 2],
}

/// How often "Replay session" adds the next item.
const REPLAY_STEP: Duration = Duration::from_millis(400);

struct Replay {
    shown: usize,
    _timer: Task<()>,
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let story = std::env::var("GALLERY_STORY")
            .ok()
            .and_then(|name| Story::ALL.into_iter().find(|s| s.title().eq_ignore_ascii_case(&name)))
            .unwrap_or(Story::AgentPanel);
        let prompt = cx.new(|cx| {
            let mut input = PromptInput::new(
                "Ask the agent to do something…",
                "Review the current implementation and suggest the next improvement.",
                window,
                cx,
            )
            .models(preview_models())
            .model("gpt-5.2")
            .actions(preview_actions());
            input.set_context(106_300, 300_000, cx);
            input.set_context_parts(preview_context_parts(), cx);
            input.set_context_open(std::env::var("GALLERY_OPEN").is_ok(), cx);
            input
        });
        let panel_prompt = cx.new(|cx| {
            let mut input = PromptInput::new("Ask Claude Code", "", window, cx)
                .models(vec![PromptModel::new("sonnet-5", "Sonnet 5")])
                .model("sonnet-5");
            input.set_running(true, cx);
            input.set_context(172_000, 200_000, cx);
            input.set_context_open(std::env::var("GALLERY_OPEN").is_ok(), cx);
            input.set_queued(vec!["Then run the whole test suite".into(), "Open a PR when it is green".into()], cx);
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
        let _system = atelier_ui::watch_system(window, cx);
        let inline = CodeEditor::state("config.rs", INLINE_FILE, window, cx);
        // The user can type anywhere, so the hunks follow the rows they describe.
        let _inline_edits = cx.subscribe(&inline, |this, state, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let text = state.read(cx).value().to_string();
            if text != this.inline_text {
                this.inline_hunks = match this.inline_history.hunks_for(&text) {
                    Some(hunks) => hunks,
                    None => atelier_ui::inline_review::track_edit(&this.inline_hunks, &this.inline_text, &text),
                };
                this.inline_text = text;
                cx.notify();
            }
        });
        let editors = editor_story::EditorTabs::new(window, cx);
        let review = cx.new(|cx| review_story::ReviewStory::new(window, cx));
        let pull_request = cx.new(|cx| pr_story::PrStory::new(window, cx));
        let merge = cx.new(|cx| merge_story::MergeStory::new(window, cx));
        let mut gallery =
            Self {
            story,
            choice: None,
            inline,
            inline_hunks: inline_fixture(),
            inline_resolving: Vec::new(),
            inline_text: INLINE_FILE.to_string(),
            inline_history: Default::default(),
            _inline_edits,
            editors,
            review,
            pull_request,
            merge,
            load: None,
            agent_sidebar: None,
            pr_view: None,
            tasks: None,
            variants: None,
            streaming: None,
            motion: None,
            agent_panels: None,
            agent_replay: None,
            voice: None,
            focus: cx.focus_handle(), prompt, panel_prompt, notice: None, started: Instant::now(), replay: None, replays: 0, tick: 0, live: None, _system };
        if gallery.story == Story::Editor {
            gallery.editors.open(cx);
        }
        gallery.open_load(window, cx);
        window.focus(&gallery.focus, cx);
        if std::env::var("GALLERY_REPLAY").is_ok_and(|v| v == "1") {
            gallery.start_replay(cx);
        }
        if std::env::var("GALLERY_LIVE").is_ok_and(|v| v == "1") {
            gallery.toggle_live(cx);
        }
        gallery
    }

    /// Builds the Highlight load story the first time it is picked.
    fn open_load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.story == Story::Load && self.load.is_none() {
            self.load = Some(cx.new(|cx| load_story::LoadStory::new(window, cx)));
        }
        if self.story == Story::AgentSidebar && self.agent_sidebar.is_none() {
            self.agent_sidebar = Some(cx.new(|cx| sidebar_story::SidebarStory::new(window, cx)));
        }
        if self.story == Story::Motion && self.motion.is_none() {
            self.motion = Some(cx.new(|cx| motion_story::MotionStory::new(window, cx)));
        }
        if self.story == Story::Streaming && self.streaming.is_none() {
            self.streaming = Some(cx.new(|cx| streaming_story::StreamingStory::new(window, cx)));
        }
        if self.story == Story::Variants && self.variants.is_none() {
            self.variants = Some(cx.new(|cx| variants_story::VariantsStory::new(window, cx)));
        }
        if self.story == Story::Tasks && self.tasks.is_none() {
            self.tasks = Some(cx.new(|cx| tasks_story::TasksStory::new(window, cx)));
        }
        if self.story == Story::PullRequestView && self.pr_view.is_none() {
            self.pr_view = Some(cx.new(|cx| pr_view_story::PrViewStory::new(window, cx)));
        }
        if self.story == Story::AgentPanels && self.agent_panels.is_none() {
            self.agent_panels = Some(cx.new(|cx| panels_story::PanelsStory::new(window, cx)));
        }
        if self.story == Story::Voice && self.voice.is_none() {
            self.voice = Some(cx.new(|cx| voice_story::VoiceStory::new(window, cx)));
        }
        if self.story == Story::AgentReplay && self.agent_replay.is_none() {
            self.agent_replay = Some(cx.new(|cx| replay_story::ReplayStory::new(window, cx)));
        }
    }

    fn is_live(&self) -> bool {
        self.live.is_some()
    }

    /// Starts or stops the live clock.
    fn toggle_live(&mut self, cx: &mut Context<Self>) {
        if self.live.take().is_none() {
            self.live = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(agent_parts::LIVE_STEP).await;
                    if this.update(cx, |g, cx| {
                        g.tick += 1;
                        cx.notify();
                    })
                    .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        cx.notify();
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
                        replay.shown < agent_panel::SESSION_LEN
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
                    .child("atelier-ui for atelier"),
            )
            // The list scrolls, so the theme picker at the foot stays in view in a short window.
            .child(div().id("stories").flex().flex_col().gap(px(2.)).flex_1().min_h_0().overflow_y_scroll().children(Story::ALL.into_iter().map(|story| {
                let selected = story == self.story;
                let hover = theme.muted_hover();
                div()
                    .id(story.title())
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(atelier_ui::theme::radius::lg())
                    .cursor_pointer()
                    .text_size(TextSize::Sm.font_size())
                    .when(selected, |d| d.bg(theme.card_strong).font_weight(FontWeight::MEDIUM))
                    .when(!selected, |d| d.text_color(theme.muted_foreground).hover(move |s| s.bg(hover)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.story = story;
                        if story == Story::Editor {
                            this.editors.open(cx);
                        }
                        this.open_load(window, cx);
                        cx.notify();
                    }))
                    .child(story.title())
            })))
            .child(div().flex_none().pt(px(8.)).debug_selector(|| "theme-picker".into()).child(theme_picker(&theme)))
    }

    fn story(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.story {
            Story::AgentPanel => {
                let shown = self.replay.as_ref().map_or(agent_panel::SESSION_LEN, |r| r.shown);
                agent_panel::agent_panel(&self.panel_prompt, self.started, self.replays, shown, self.tick, cx).into_any_element()
            }
            Story::ChangedFiles => agent_parts::changed_files_story(self.tick, self.is_live(), cx).into_any_element(),
            Story::Worktrees => agent_parts::worktrees_story().into_any_element(),
            Story::Providers => providers_story::providers_story(cx).into_any_element(),
            Story::ProviderSettings => providers_story::settings_story(cx).into_any_element(),
            Story::SignInNotice => sign_in_story::sign_in_story(cx).into_any_element(),
            Story::SubagentCard => agent_parts::subagent_card_story(self.tick, self.is_live(), cx).into_any_element(),
            Story::SubagentStrip => agent_parts::subagent_strip_story(self.tick, self.is_live(), cx).into_any_element(),
            Story::PrCard => agent_parts::pr_card_story().into_any_element(),
            Story::PrChip => agent_parts::pr_chip_story().into_any_element(),
            Story::ModelBadge => agent_parts::model_badge_story().into_any_element(),
            Story::Review => review_story::element(&self.review),
            Story::PullRequest => pr_story::element(&self.pull_request),
            Story::Load => self.load.clone().map(|s| s.into_any_element()).unwrap_or_else(|| div().into_any_element()),
            Story::AgentSidebar => self.agent_sidebar.clone().map(|s| s.into_any_element()).unwrap_or_else(|| div().into_any_element()),
            Story::AgentPanels => self.agent_panels.clone().map(|s| s.into_any_element()).unwrap_or_else(|| div().into_any_element()),
            Story::Tasks => self.tasks.clone().map(|s| s.into_any_element()).unwrap_or_else(|| div().into_any_element()),
            Story::Variants => self.variants.clone().map(|s| s.into_any_element()).unwrap_or_else(|| div().into_any_element()),
            Story::Streaming => self.streaming.clone().map(|s| s.into_any_element()).unwrap_or_else(|| div().into_any_element()),
            Story::Motion => self.motion.clone().map(|s| s.into_any_element()).unwrap_or_else(|| div().into_any_element()),
            Story::PullRequestView => self.pr_view.clone().map(|s| s.into_any_element()).unwrap_or_else(|| div().into_any_element()),
            Story::Voice => self.voice.clone().map(|s| s.into_any_element()).unwrap_or_else(|| div().into_any_element()),
            Story::AgentReplay => self.agent_replay.clone().map(|s| s.into_any_element()).unwrap_or_else(|| div().into_any_element()),
            Story::PullRequests => pr_story::pull_requests().into_any_element(),
            Story::Merge => self.merge.clone().into_any_element(),
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
            Story::Inline => inline_story(&self.inline, &self.inline_hunks, &self.inline_resolving, cx).into_any_element(),
            Story::Editor => editor_story::editor_story(self, cx).into_any_element(),
            Story::Select => select_story(self.choice, cx).into_any_element(),
            Story::Prompt => prompt_story(&self.prompt, self.notice.clone(), cx).into_any_element(),
        }
    }
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        // These draw edge to edge, as a real pane does, with no title or padding around them.
        let panel = matches!(self.story, Story::AgentPanel | Story::PullRequest | Story::PullRequests);
        div()
            .track_focus(&self.focus)
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
                    // A flex child is as wide as its content by default; this one is the window's.
                    .min_w_0()
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
            .child(div().h(px(64.)).rounded(atelier_ui::theme::radius::xl()).bg(color))
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
                .child("cargo test -p ui --release")
                .child("crates/ui/src/file_diff.rs  +42 -7")
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
            .child(claude::spark(SharedString::from(format!("spark-{}", state.name())), state).size(px(20.)))
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
            "Sizes: Small (the default), Medium for an action that must stand out, Large and Xl",
            row()
                .child(Button::new("size-sm").label("Small"))
                .child(Button::new("size-md").label("Medium").size(ButtonSize::Md))
                .child(Button::new("size-lg").label("Large").size(ButtonSize::Lg))
                .child(Button::new("size-xl").label("Xl").size(ButtonSize::Xl)),
        ))
        .child(section(
            "A button group in each size",
            row().children([("Sm", ButtonSize::Sm), ("Md", ButtonSize::Md), ("Lg", ButtonSize::Lg), ("Xl", ButtonSize::Xl)].map(|(name, size)| {
                atelier_ui::ButtonGroup::new(SharedString::from(format!("group-size-{name}")))
                    .size(size)
                    .child(Button::new(SharedString::from(format!("gs-{name}-merge"))).label("Squash and merge"))
                    .child(Button::new(SharedString::from(format!("gs-{name}-more"))).icon(IconName::ChevronDown))
            })),
        ))
        .child(section(
            "With a chip: hover to slide the arrow",
            row()
                .child(Button::new("chip-sm").label("Add").chip(IconName::ArrowForward))
                .child(Button::new("chip-md").label("New session").chip(IconName::ArrowForward).size(ButtonSize::Md))
                .child(Button::new("chip-lg").label("Continue").chip(IconName::ArrowForward).size(ButtonSize::Lg))
                .child(Button::new("chip-xl").label("Get started").chip(IconName::ArrowForward).size(ButtonSize::Xl)),
        ))
        .child(section(
            "Icon buttons: Icon (28) and IconSm (24), beside Small and Medium text buttons",
            row()
                .child(Button::new("i-send").icon(IconName::ArrowUp).size(ButtonSize::Icon))
                .child(Button::new("i-stop").icon(IconName::Square).size(ButtonSize::Icon).variant(ButtonVariant::Secondary))
                .child(Button::new("i-attach").icon(IconName::AttachFile).size(ButtonSize::Icon).variant(ButtonVariant::Ghost))
                .child(Button::new("i-more").icon(IconName::MoreHoriz).size(ButtonSize::Icon).variant(ButtonVariant::Ghost))
                .child(Button::new("i-text-sm").label("Small").variant(ButtonVariant::Secondary))
                .child(Button::new("is-more").icon(IconName::MoreHoriz).size(ButtonSize::IconSm).variant(ButtonVariant::Ghost))
                .child(Button::new("is-add").icon(IconName::Add).size(ButtonSize::IconSm).variant(ButtonVariant::Ghost))
                .child(Button::new("is-fill").icon(IconName::Close).size(ButtonSize::IconSm).variant(ButtonVariant::Secondary))
                .child(Button::new("i-text-md").label("Medium").size(ButtonSize::Md).variant(ButtonVariant::Secondary)),
        ))
        .child(section("Disabled", row().child(Button::new("disabled").label("Send").disabled(true))))
        .child(section(
            "Button groups: two and three parts, primary and secondary, one part disabled",
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    row()
                        .child(
                            atelier_ui::ButtonGroup::new("group-two")
                                .child(Button::new("g2-merge").label("Squash and merge"))
                                .child(Button::new("g2-more").icon(IconName::ChevronDown)),
                        )
                        .child(
                            atelier_ui::ButtonGroup::new("group-three")
                                .child(Button::new("g3-prev").icon(IconName::ArrowUp))
                                .child(Button::new("g3-mark").label("Seen"))
                                .child(Button::new("g3-next").icon(IconName::ArrowDown)),
                        ),
                )
                .child(
                    row()
                        .child(
                            atelier_ui::ButtonGroup::new("group-two-secondary")
                                .variant(ButtonVariant::Secondary)
                                .child(Button::new("g2s-open").label("Open"))
                                .child(Button::new("g2s-more").icon(IconName::ChevronDown)),
                        )
                        .child(
                            atelier_ui::ButtonGroup::new("group-three-secondary")
                                .variant(ButtonVariant::Secondary)
                                .child(Button::new("g3s-left").label("Left"))
                                .child(Button::new("g3s-center").label("Center"))
                                .child(Button::new("g3s-right").label("Right")),
                        ),
                )
                .child(
                    row().child(
                        atelier_ui::ButtonGroup::new("group-disabled")
                            .child(Button::new("gd-merge").label("Squash and merge").disabled(true).tooltip("2 checks still running"))
                            .child(Button::new("gd-more").icon(IconName::ChevronDown)),
                    ),
                ),
        ))
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
The fix is in `crates/ui/src/file_diff.rs`.";

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
        .child(Thinking::new("think", claude::look(), ThinkingPhase::Thinking { since: started }).loading(claude::loading_strips()).elapsed("4s").tokens(1_234))
        .child(
            Thinking::new("think-stepped", claude::look(), ThinkingPhase::Thinking { since: started }).loading(claude::loading_strips())
                .style(ThinkingStyle::Shimmer(Shimmer::Stepped))
                .elapsed("4s"),
        )
        .child(Thinking::new("think-breath", claude::look(), thinking_for(started, 16)).loading(claude::loading_strips()).style(ThinkingStyle::Breath).elapsed("16s"))
        .child(Thinking::new("sending", claude::look(), ThinkingPhase::Sending).loading(claude::loading_strips()))
        .child(Thinking::new("tools", claude::look(), ThinkingPhase::RunningTools).loading(claude::loading_strips()).elapsed("31s").tasks(3))
        .child(Thinking::new("orbit", claude::look(), ThinkingPhase::Thinking { since: started }).loading(claude::loading_strips()).elapsed("9s").subagents(2))
        .child(Thinking::new("thought", claude::look(), ThinkingPhase::Thought { seconds: 4 }))
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
                    .child(ToolCall::new("t-read", "Read file").file("crates/ui/src/file_diff.rs").meta("214 lines").status(ToolStatus::Done).flat())
                    .child(ToolCall::new("t-grep", "Searched code").tool("fn hunk_starts").status(ToolStatus::Done).flat().output("crates/ui/src/file_diff.rs:69: fn hunk_starts(header: &str) -> (u32, u32) {"))
                    .child(ToolCall::new("t-test", "Ran tests").tool("cargo test -p ui").meta("3.1s").status(ToolStatus::Done).output(TEST_OUTPUT))
                    .child(ToolCall::new("t-run", "Running clippy").tool("cargo clippy --workspace").status(ToolStatus::Running).output("Checking ui v0.1.0\n    Checking atelier-gallery v0.1.0"))
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
                        ToolApproval::new("approve-edit", "Edit")
                            .title("Edit src/main.rs")
                            .preview(atelier_ui::ToolPreview::edit(
                                "src/main.rs",
                                "}\n\nfn main() {\n    println!(\"{}\", add(2, 3));\n}\n",
                                "}\n\nfn subtract(a: i32, b: i32) -> i32 {\n    a - b\n}\n\nfn main() {\n    println!(\"{}\", add(2, 3));\n    println!(\"{}\", subtract(5, 2));\n}\n",
                            ))
                            .parameter("file_path", "/tmp/atelier-ux/scratch/src/main.rs")
                            .on_approve(|_, _, _| {})
                            .on_always_allow(|_, _, _| {})
                            .on_deny(|_, _, _| {}),
                    )
                    .child(
                        ToolApproval::new("approve-command", "Bash")
                            .preview(atelier_ui::ToolPreview::command("cargo test --workspace -- --nocapture"))
                            .parameter("command", "cargo test --workspace -- --nocapture")
                            .on_approve(|_, _, _| {})
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
--- a/crates/ui/src/file_diff.rs
+++ b/crates/ui/src/file_diff.rs
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
                        FileDiff::new("diff-a", "crates/ui/src/file_diff.rs", diff_a)
                            .default_open(true)
                            .status(FileDiffStatus::Complete)
                            .copy_text(diff_a_copy),
                    )
                    .child(
                        FileDiff::new(
                            "diff-b",
                            "crates/ui/src/file_diff/tests.rs",
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


/// Both sides of two hunks, as real text in one buffer. Rows 1 and 8 are the old code; rows 2 to 3
/// and row 9 are the agent's. Nothing is virtual, which is why the buffer stays writable.
const INLINE_FILE: &str = "pub struct Config {\n    pub width: u32,\n    pub width: u32,\n    pub height: u32,\n}\n\nimpl Config {\n    pub fn new() -> Self {\n        Self { width: 80 }\n        Self { width: 80, height: 24 }\n    }\n}\n";

fn inline_fixture() -> Vec<InlineHunk> {
    vec![InlineHunk::new("field", 1..2, 2..4), InlineHunk::new("ctor", 8..9, 9..10)]
}

fn inline_story(
    state: &Entity<gpui_kit::component::input::EditorState>,
    hunks: &[InlineHunk],
    resolving: &[atelier_ui::Resolve],
    cx: &mut Context<Gallery>,
) -> impl IntoElement {
    let left = atelier_ui::inline_review::pending_count(hunks, &[]);
    // A decision starts the hunk's fade; its edit runs once the fade is over.
    let decide = cx.listener(|this: &mut Gallery, (id, decision): &(SharedString, Decision), _, cx| {
        let (now, reduce_motion) = (std::time::Instant::now(), cx.reduce_motion());
        this.inline_resolving.retain(|r| !r.is_over(now, reduce_motion));
        if !this.inline_resolving.iter().any(|r| &r.id == id) {
            this.inline_resolving.push(atelier_ui::Resolve::new(id.clone(), *decision));
            cx.notify();
        }
    });
    let resolved = cx.listener(
        |this: &mut Gallery, (id, decision): &(SharedString, Decision), window: &mut Window, cx| {
            // A frame can report the same finished fade twice; only the first one edits.
            let Some(at) = this.inline_resolving.iter().position(|r| &r.id == id && !r.is_edited()) else { return };
            let Some(hunk) = this.inline_hunks.iter().find(|h| &h.id == id).cloned() else {
                this.inline_resolving.remove(at);
                return;
            };
            let closed = hunk.closing(*decision);
            // The resolve stays until its gap has closed; the next decision drops it.
            this.inline_resolving[at].edited(closed.clone());
            let before = (this.inline_text.clone(), this.inline_hunks.clone());
            atelier_ui::inline_review::apply(&this.inline, &[(hunk, *decision)], window, cx);
            this.inline_hunks = atelier_ui::inline_review::shift_after(&this.inline_hunks, id, &closed);
            // The decision already moved the hunks; the edit it made must not move them again.
            this.inline_text = this.inline.read(cx).value().to_string();
            this.inline_history.record(before, (this.inline_text.clone(), this.inline_hunks.clone()));
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
                .resolving(resolving.to_vec())
                .on_decide(move |id, decision, window, cx| decide(&(id.clone(), decision), window, cx))
                .on_resolved(move |id, decision, window, cx| resolved(&(id.clone(), decision), window, cx)),
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

/// The theme picker at the sidebar's foot. A pick is remembered for the next launch.
fn theme_picker(theme: &atelier_ui::Theme) -> impl IntoElement {
    atelier_ui::theme_picker::theme_picker("theme", theme, |picked, cx| {
        let name = picked.name.to_string();
        if let Some(path) = atelier_settings::path() {
            cx.background_spawn(async move {
                if let Err(error) = atelier_settings::update(&path, |s| s.theme = Some(name)) {
                    eprintln!("could not save the theme: {error}");
                }
            })
            .detach();
        }
    })
}

/// The theme to start in: `GALLERY_THEME`, then the one saved last time; `None` follows the system.
fn starting_theme(saved: &atelier_settings::Settings) -> Option<atelier_ui::Theme> {
    let by_name = |name: &str| match name {
        "light" => Some(atelier_ui::themes::atelier(Appearance::Light).clone()),
        "dark" => Some(atelier_ui::themes::atelier(Appearance::Dark).clone()),
        name => atelier_ui::themes::named(name).cloned(),
    };
    match std::env::var("GALLERY_THEME") {
        Ok(name) => by_name(&name).or_else(|| {
            eprintln!("GALLERY_THEME {name:?} names no theme");
            None
        }),
        Err(_) => saved.theme.as_deref().and_then(by_name),
    }
}

fn main() {
    // Read before the event loop starts, so the UI thread never waits on the disk.
    let saved = atelier_settings::path().map(|p| atelier_settings::load(&p)).unwrap_or_default();
    gpui_kit::application().with_assets(atelier_agents::Assets).run(move |cx| {
        atelier_ui::init(cx);
        if let Some(theme) = starting_theme(&saved) {
            atelier_ui::theme::set_theme(theme, cx);
        }
        // GALLERY_SIZE=1500x900 opens a larger window, for a story laid out like a full screen.
        let (w, h) = std::env::var("GALLERY_SIZE")
            .ok()
            .and_then(|v| v.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))))
            .unwrap_or((1100., 860.));
        let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
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

#[cfg(test)]
mod tests;
