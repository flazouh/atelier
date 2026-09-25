//! Shows every beui component in each state, in light and dark. Run with `cargo run -p beui-gallery`.
//! `GALLERY_STORY=<title>` opens a story and `GALLERY_THEME=light|dark` overrides the system theme, so a script can screenshot them.

use beui::{
    ActiveTheme, AgentText, Appearance, Badge, Button, ButtonSize, ButtonVariant, CodeBlock, DiffLine, FONT_FAMILY,
    FileDiff, Icon, IconName, Kbd, MONO_FONT_FAMILY, MessageBubble, PromptInput, PromptInputEvent, Spinner, TextSize,
    Thinking, Todo, TodoList, TodoStatus, ToolApproval, ToolCall, ToolKind, ToolStatus, Tone, pane_header,
};
use gpui_kit::{
    AnyElement, App, AppContext, Bounds, Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds, WindowOptions, div,
    prelude::FluentBuilder, px, size,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Story {
    AgentPanel,
    Colors,
    Typography,
    Icons,
    Buttons,
    Badges,
    Messages,
    Tools,
    Diffs,
    Plan,
    Prompt,
}

impl Story {
    const ALL: [Story; 11] = [
        Story::AgentPanel,
        Story::Colors,
        Story::Typography,
        Story::Icons,
        Story::Buttons,
        Story::Badges,
        Story::Messages,
        Story::Tools,
        Story::Diffs,
        Story::Plan,
        Story::Prompt,
    ];

    fn title(self) -> &'static str {
        match self {
            Story::AgentPanel => "Agent panel",
            Story::Colors => "Colors",
            Story::Typography => "Typography",
            Story::Icons => "Icons",
            Story::Buttons => "Buttons",
            Story::Badges => "Badges and keys",
            Story::Messages => "Messages",
            Story::Tools => "Tool calls",
            Story::Diffs => "Diffs and code",
            Story::Plan => "Plan",
            Story::Prompt => "Prompt input",
        }
    }
}

struct Gallery {
    story: Story,
    prompt: Entity<PromptInput>,
    panel_prompt: Entity<PromptInput>,
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let story = std::env::var("GALLERY_STORY")
            .ok()
            .and_then(|name| Story::ALL.into_iter().find(|s| s.title().eq_ignore_ascii_case(&name)))
            .unwrap_or(Story::AgentPanel);
        let prompt = cx.new(|cx| PromptInput::new("Ask Claude Code", "Sonnet 5", window, cx));
        let panel_prompt = cx.new(|cx| {
            let mut input = PromptInput::new("Ask Claude Code", "Sonnet 5", window, cx);
            input.set_running(true, cx);
            input
        });
        // The gallery only prints what the input asked for; the app will act on it.
        for input in [&prompt, &panel_prompt] {
            cx.subscribe(input, |_, _, event: &PromptInputEvent, _| println!("prompt: {event:?}")).detach();
        }
        Self { story, prompt, panel_prompt }
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
            .border_r_1()
            .border_color(theme.border)
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
                    .when(selected, |d| d.bg(theme.card).font_weight(FontWeight::MEDIUM))
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
            Story::AgentPanel => agent_panel(&self.panel_prompt, cx).into_any_element(),
            Story::Colors => colors(cx).into_any_element(),
            Story::Typography => typography().into_any_element(),
            Story::Icons => icons(cx).into_any_element(),
            Story::Buttons => buttons().into_any_element(),
            Story::Badges => badges().into_any_element(),
            Story::Messages => messages().into_any_element(),
            Story::Tools => tools().into_any_element(),
            Story::Diffs => diffs().into_any_element(),
            Story::Plan => narrow(TodoList::new("plan", sample_plan())).into_any_element(),
            Story::Prompt => narrow(self.prompt.clone()).into_any_element(),
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
    let swatches: [(&str, Hsla); 14] = [
        ("background", t.background),
        ("foreground", t.foreground),
        ("card", t.card),
        ("muted foreground", t.muted_foreground),
        ("border", t.border),
        ("border strong", t.border_strong),
        ("primary", t.primary),
        ("primary foreground", t.primary_foreground),
        ("accent", t.accent),
        ("danger", t.danger),
        ("success", t.success),
        ("warning", t.warning),
        ("diff added", t.diff_line(true)),
        ("diff removed", t.diff_line(false)),
    ];
    let border = t.border_strong;
    row().children(swatches.into_iter().map(move |(name, color)| {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w(px(120.))
            .child(div().h(px(64.)).rounded(beui::theme::radius::XL).bg(color).border_1().border_color(border))
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
                    .child(format!("{name} · Refactor the parser to stream tokens."))
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

fn buttons() -> impl IntoElement {
    let variants =
        [("Primary", ButtonVariant::Primary), ("Secondary", ButtonVariant::Secondary), ("Ghost", ButtonVariant::Ghost), ("Outline", ButtonVariant::Outline)];
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
                .child(Button::new("size-sm").label("Small").size(ButtonSize::Sm))
                .child(Button::new("size-md").label("Medium"))
                .child(Button::new("size-lg").label("Large").size(ButtonSize::Lg)),
        ))
        .child(section(
            "With a chip: hover to slide the arrow",
            row()
                .child(Button::new("chip-md").label("New session").chip(IconName::ArrowRight))
                .child(Button::new("chip-sm").label("Continue").chip(IconName::ArrowRight).size(ButtonSize::Sm)),
        ))
        .child(section(
            "Icon buttons",
            row()
                .child(Button::new("i-send").icon(IconName::ArrowUp).size(ButtonSize::Icon))
                .child(Button::new("i-stop").icon(IconName::Stop).size(ButtonSize::Icon).variant(ButtonVariant::Secondary))
                .child(Button::new("i-attach").icon(IconName::Attachment).size(ButtonSize::Icon).variant(ButtonVariant::Ghost))
                .child(Button::new("i-more").icon(IconName::More).size(ButtonSize::Icon).variant(ButtonVariant::Outline)),
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

fn messages() -> impl IntoElement {
    narrow(
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(section("User", MessageBubble::new("The line numbers in the diff view are off by one. Can you fix it?")))
            .child(section("Agent", AgentText::new("reply", REPLY)))
            .child(section("Working", div().flex().flex_col().child(Thinking::new("think", "Thinking").elapsed("4s")).child(Thinking::new("run", "Running tests")))),
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
                    .child(ToolCall::new("t-read", ToolKind::Read, "Read").summary("crates/beui/src/file_diff.rs").meta("214 lines").status(ToolStatus::Done))
                    .child(ToolCall::new("t-grep", ToolKind::Search, "Grep").summary("fn hunk_starts").status(ToolStatus::Done).output("crates/beui/src/file_diff.rs:69: fn hunk_starts(header: &str) -> (u32, u32) {"))
                    .child(ToolCall::new("t-test", ToolKind::Shell, "Bash").summary("cargo test -p beui").meta("3.1s").status(ToolStatus::Done).output(TEST_OUTPUT))
                    .child(ToolCall::new("t-run", ToolKind::Shell, "Bash").summary("cargo clippy --workspace").status(ToolStatus::Running))
                    .child(ToolCall::new("t-fail", ToolKind::Web, "WebFetch").summary("https://beui.dev/docs/theme").status(ToolStatus::Failed).output("error: request timed out after 30s"))
                    .child(ToolCall::new("t-wait", ToolKind::Shell, "Bash").summary("git push origin main").status(ToolStatus::Pending)),
            ))
            .child(section(
                "Approval",
                ToolApproval::new("approve-push", "Run a shell command", "git push origin main")
                    .icon(IconName::Terminal)
                    .reason("Push the fix so CI can run the full test suite.")
                    .detail("Directory", "~/Documents/lathe")
                    .detail("Command", "git push origin main"),
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
+            .map_or(0, |n| n.saturating_sub(1))
     };
     (start('-'), start('+'))
 }";

const CODE: &str = "#[test]\nfn a_hunk_at_line_one_starts_at_one() {\n    let lines = DiffLine::parse(\"@@ -1,1 +1,1 @@\\n a\");\n    assert_eq!(lines[1].number, Some(1));\n}";

fn diffs() -> impl IntoElement {
    narrow(
        div()
            .flex()
            .flex_col()
            .child(section(
                "File diff",
                div()
                    .flex()
                    .flex_col()
                    .child(FileDiff::new("diff-a", "crates/beui/src/file_diff.rs", DiffLine::parse(DIFF)).default_open(true))
                    .child(FileDiff::new("diff-b", "crates/beui/src/file_diff/tests.rs", DiffLine::parse("@@ -40,0 +41,6 @@\n+#[test]\n+fn one() {}\n"))),
            ))
            .child(section("Code block", CodeBlock::new("code", CODE).title("file_diff/tests.rs").language("rust"))),
    )
}

fn sample_plan() -> Vec<Todo> {
    vec![
        Todo::new("Find where hunk numbers start", TodoStatus::Done),
        Todo::new("Fix the off-by-one in hunk_starts", TodoStatus::Done),
        Todo::new("Add a test for a hunk at line 1", TodoStatus::InProgress),
        Todo::new("Run the full test suite", TodoStatus::Pending),
    ]
}

/// A whole session as the panel will show it, at the panel's width.
fn agent_panel(prompt: &Entity<PromptInput>, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let header = pane_header("Claude Code", cx)
        .child(Badge::new("Sonnet 5"))
        .child(div().flex_1())
        .child(Button::new("p-new").icon(IconName::Plus).variant(ButtonVariant::Ghost).size(ButtonSize::Icon))
        .child(Button::new("p-more").icon(IconName::More).variant(ButtonVariant::Ghost).size(ButtonSize::Icon));

    let session = div()
        .id("session")
        .flex_1()
        .overflow_y_scroll()
        .px(px(20.))
        .py(px(20.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(MessageBubble::new("The line numbers in the diff view are off by one. Can you fix it?"))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(ToolCall::new("s-read", ToolKind::Read, "Read").summary("crates/beui/src/file_diff.rs").meta("214 lines").status(ToolStatus::Done))
                        .child(ToolCall::new("s-grep", ToolKind::Search, "Grep").summary("fn hunk_starts").status(ToolStatus::Done)),
                )
                .child(AgentText::new("s-reply", REPLY))
                .child(TodoList::new("s-plan", sample_plan()))
                .child(FileDiff::new("s-diff", "crates/beui/src/file_diff.rs", DiffLine::parse(DIFF)))
                .child(ToolCall::new("s-test", ToolKind::Shell, "Bash").summary("cargo test -p beui").meta("3.1s").status(ToolStatus::Done).output(TEST_OUTPUT))
                .child(
                    ToolApproval::new("s-push", "Run a shell command", "git push origin main")
                        .icon(IconName::Terminal)
                        .reason("Push the fix so CI can run the full test suite.")
                        .detail("Directory", "~/Documents/lathe"),
                )
                .child(Thinking::new("s-think", "Waiting for approval").elapsed("18s")),
        );

    div()
        .flex()
        .justify_center()
        .size_full()
        .bg(theme.card.opacity(0.4))
        .child(
            div()
                .flex()
                .flex_col()
                .w(px(560.))
                .h_full()
                .bg(theme.background)
                .border_l_1()
                .border_r_1()
                .border_color(theme.border)
                .child(header)
                .child(session)
                .child(div().p(px(12.)).child(prompt.clone())),
        )
}

fn main() {
    gpui_kit::application().with_assets(beui::Assets).run(|cx| {
        beui::init(cx);
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
