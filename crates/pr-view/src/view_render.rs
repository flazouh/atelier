//! How the pull request view draws: a rail on the left, and on the right the seen bar, the tree and the
//! diff. The layout is the pull request story's (see [`crate::layout`]); the data is the model's.
use beui::{
    Segment, Segmented,
    ActiveTheme, AgentText, Button, ButtonSize, ButtonVariant, ChangedFileTree, ChecksPanel, CommitsSummary, ConversationList, InlineReview, ReviewBar,
    ReviewFileHeader, UnsentComments,
    theme::radius,
    typography::TextSize,
};
use gpui_kit::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use lathe_forge::{ForgeError, PullState};

use crate::{
    data::PartKind,
    diff::Content,
    layout::{PADDING, Part, RAIL_GAP, RAIL_MAX, TREE},
    place::place,
    services::now,
    view::PullView,
};

/// The description shows this tall until the reader asks for all of it.
const BODY_CLIP: f32 = 160.;

/// Whether a description is long enough to be clipped at [`BODY_CLIP`], so it needs the "Show the whole description"
/// button: about seven lines of a rail's width, or more than six lines of its own.
pub(crate) fn body_needs_fold(body: &str) -> bool {
    let lines: usize = body.lines().map(|l| l.chars().count().div_ceil(48).max(1)).sum();
    lines > 6
}

/// The height of the fade at the foot of the rail while it has more to show.
const RAIL_FADE: f32 = 36.;

/// The height of the switch between the two parts in a narrow pane.
const PARTS_HEIGHT: f32 = 46.;

/// The line under the diff.
const STATUS_HEIGHT: f32 = 26.;

fn state_word(state: PullState) -> (&'static str, bool) {
    match state {
        PullState::Open => ("Open", false),
        PullState::Draft => ("Draft", false),
        PullState::Merged => ("Merged", false),
        PullState::Closed => ("Closed", true),
    }
}

impl PullView {
    fn header(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(pull) = self.model.pull().cloned() else { return div().into_any_element() };
        let (state, bad) = state_word(pull.state);
        let base_words: SharedString = match &self.model.base {
            Some(base) => match &base.choice {
                crate::base::BaseChoice::Whole => "the whole pull request".into(),
                crate::base::BaseChoice::LastReview => "since your last review".into(),
                crate::base::BaseChoice::Commit(sha) => format!("since {}", crate::git::short(sha)).into(),
            },
            None => "…".into(),
        };
        let note = self.header_note();
        let error = !self.model.errors.is_empty() || self.model.git_error.is_some();
        let body: SharedString = if pull.body.chars().count() > 1500 { format!("{}…", pull.body.chars().take(1500).collect::<String>()).into() } else { pull.body.clone().into() };
        let pick = cx.listener(|view, _, window, cx| view.pick_base(window, cx));
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(6.))
            .p(px(12.))
            .rounded(radius::LG)
            .bg(theme.card)
            .child(div().font_weight(FontWeight::SEMIBOLD).text_color(theme.foreground).child(SharedString::from(pull.title.clone())))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .text_size(TextSize::Xs.font_size())
                    .text_color(muted)
                    .child(div().text_color(if bad { theme.danger } else { theme.foreground }).child(state))
                    .child(format!("#{} by {}", pull.reference.number, pull.author))
                    .child(format!("{} into {}", pull.head, pull.base)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(TextSize::Xs.font_size())
                    .text_color(muted)
                    .child("Showing")
                    .child(Button::new("pr-base").label(base_words).variant(ButtonVariant::Secondary).size(ButtonSize::Sm).on_click(move |e, w, cx| pick(e, w, cx))),
            )
            .when_some(note, |d, note| d.child(div().text_size(TextSize::Xs.font_size()).text_color(if error { theme.danger } else { muted }).child(SharedString::from(note))))
            .when(!pull.body.trim().is_empty(), |d| {
                let open = self.body_open;
                let needs_fold = body_needs_fold(&pull.body);
                let toggle = cx.listener(|view, _, _, cx| {
                    view.body_open = !view.body_open;
                    cx.notify();
                });
                d.child(
                    div()
                        .pt(px(4.))
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(div().when(!open, |d| d.max_h(px(BODY_CLIP)).overflow_hidden()).child(AgentText::new("pr-body", body)))
                        .when(needs_fold || open, |d| {
                            d.child(Button::new("pr-body-more").label(if open { "Show less" } else { "Show the whole description" }).variant(ButtonVariant::Ghost).size(ButtonSize::Sm).on_click(move |e, w, cx| toggle(e, w, cx)))
                        }),
                )
            })
            .into_any_element()
    }

    fn rail(&mut self, height: f32, width: f32, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let now = now();
        let page = self.model.conversation_page(now, self.page.0, self.page.1);
        let commits = crate::present::commit_data(&self.model.commits, now);
        let checks = self.model.check_rows();
        let unsent = self.model.unsent();
        let this = cx.entity().downgrade();
        let (weak_open, weak_reply, weak_resolve, weak_send) = (this.clone(), this.clone(), this.clone(), this.clone());
        let listed: Vec<&lathe_forge::Thread> = self.model.threads_in_list_order().into_iter().take(self.page.0).collect();
        let ordered: Vec<lathe_forge::ThreadId> = listed.iter().map(|t| t.id.clone()).collect();
        let (for_open, for_reply, for_resolve) = (ordered.clone(), ordered.clone(), ordered);
        let paths: Vec<String> = listed.iter().map(|t| t.path.clone()).collect();
        let (more_threads, more_remarks) = (this.clone(), this.clone());
        let header = self.header(cx);
        let scroller = div()
            .id("pr-rail")
            .debug_selector(|| "pr-rail".into())
            .track_scroll(&self.rail_scroll)
            .flex()
            .flex_col()
            .flex_none()
            .w(px(width))
            .h(px(height))
            .gap(px(8.))
            .overflow_y_scroll()
            .child(header)
            .when(unsent > 0, |d| {
                d.child(UnsentComments::new("pr-unsent", unsent).on_send(move |_, cx| {
                    weak_send.update(cx, |view, cx| view.send_unsent(cx)).ok();
                }))
            })
            .when(!checks.is_empty(), |d| d.child(ChecksPanel::new("pr-checks", checks)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .rounded(radius::LG)
                    .bg(theme.card)
                    .child(
                        ConversationList::new("pr-conversation", page.threads, page.remarks)
                            .totals(page.open, page.resolved, page.remark_total)
                            .more_threads(page.hidden_threads, move |_, cx| {
                                more_threads.update(cx, |view, cx| view.show_more(false, cx)).ok();
                            })
                            .more_remarks(page.hidden_remarks, move |_, cx| {
                                more_remarks.update(cx, |view, cx| view.show_more(true, cx)).ok();
                            })
                            .on_open(move |i, window, cx| {
                                let (Some(id), Some(path)) = (for_open.get(i.min(10_000)).cloned(), paths.get(i).cloned()) else { return };
                                let _ = id;
                                weak_open.update(cx, |view, cx| view.open_file(&path, window, cx)).ok();
                            })
                            .on_reply(move |i, window, cx| {
                                if let Some(id) = for_reply.get(i).cloned() {
                                    weak_reply.update(cx, |view, cx| view.open_reply(&id, window, cx)).ok();
                                }
                            })
                            .on_resolve(move |i, _, cx| {
                                if let Some(id) = for_resolve.get(i).cloned() {
                                    weak_resolve.update(cx, |view, cx| view.resolve(id, true, cx)).ok();
                                }
                            }),
                    )
                    .child(self.composer.clone()),
            )
            .child(self.verdict.clone())
            .child(self.merge.clone())
            .child(CommitsSummary::new("pr-commits", commits))
            ;
        // A fade at the foot while there is more below, so the cut cards do not look like the end.
        let offset = self.rail_scroll.offset().y;
        let more_below = -f32::from(offset) < f32::from(self.rail_scroll.max_offset().y) - 1.;
        div()
            .relative()
            .flex_none()
            .w(px(width))
            .h(px(height))
            .child(scroller)
            .when(more_below, |d| {
                d.child(
                    div()
                        .debug_selector(|| "pr-rail-fade".into())
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(px(RAIL_FADE))
                        .bg(gpui_kit::linear_gradient(
                            180.,
                            gpui_kit::linear_color_stop(theme.background.opacity(0.), 0.),
                            gpui_kit::linear_color_stop(theme.background, 1.),
                        )),
                )
            })
            .into_any_element()
    }

    /// What stands where the diff goes when there is no diff to draw.
    fn empty_card(&self, words: impl Into<SharedString>, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        div().flex_1().flex().items_center().justify_center().text_color(theme.muted_foreground).child(words.into()).into_any_element()
    }

    fn file_card(&mut self, body: f32, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(place_now) = self.model.place.clone() else {
            let words = if self.model.entries.is_none() && self.model.git_error.is_none() { "Reading the files…" } else { "No files changed." };
            return div().flex_1().min_w_0().h(px(body)).flex().flex_col().rounded(radius::LG).bg(theme.card).child(self.empty_card(words, cx)).into_any_element();
        };
        let handlers = self.handlers(cx);
        let view = if place_now.brought_in { None } else { self.current_view() };
        let (added, removed) = view.as_ref().and_then(|v| v.shown()).map_or((0, 0), |s| s.counts());
        let header = ReviewFileHeader::new("pr-file", place_now.path.clone(), added, removed, handlers).brought_in(place_now.brought_in);
        let status = self.session.as_ref().map(|s| s.read(cx).status().join("   ")).unwrap_or_default();
        let open_in_editor = cx.listener(|view, _, _, cx| view.open_in_editor(cx));
        let add = cx.listener(|view, row: &usize, window, cx| view.open_composer(*row, window, cx));
        let inner = match (&view, place_now.brought_in) {
            (_, true) => {
                let review = InlineReview::new("pr-diff", &self.editor, Vec::new()).decisions(false).read_only(true).on_card(true).height(px(body - 12. - 40. - STATUS_HEIGHT));
                review.into_any_element()
            }
            (None, false) => {
                let words = self.failed_or_loading(&place_now.path);
                self.empty_card(words, cx)
            }
            (Some(view), false) => match &view.content {
                Content::Binary => self.empty_card("This file is not text, so there is no diff to show.", cx),
                Content::TooLarge(size) => self.empty_card(format!("This file is {} MB, too big to diff here.", size / 1_048_576), cx),
                Content::Text(shown) => {
                    let blocks = self.row_blocks(view, cx);
                    let placement = place(&self.model.data.threads, view);
                    let listed = placement.file_level.len() + placement.outdated.len();
                    let review = InlineReview::new("pr-diff", &self.editor, shown.hunks().to_vec())
                        .decisions(false)
                        .read_only(true)
                        .on_card(true)
                        .row_blocks(blocks)
                        .on_add_comment(move |row, window, cx| add(&row, window, cx))
                        .height(px(body - 12. - 40. - STATUS_HEIGHT - if listed > 0 { 34. } else { 0. }));
                    div()
                        .flex()
                        .flex_col()
                        .when(listed > 0, |d| {
                            d.child(
                                div()
                                    .flex_none()
                                    .h(px(34.))
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .text_size(TextSize::Xs.font_size())
                                    .text_color(muted)
                                    .child(format!("{} on the whole file or on code that changed since", if listed == 1 { "1 thread".to_string() } else { format!("{listed} threads") })),
                            )
                        })
                        .child(review)
                        .into_any_element()
                }
            },
        };
        div()
            .flex_1()
            .min_w_0()
            .h(px(body))
            .bg(theme.card)
            .rounded(radius::LG)
            .p(px(6.))
            .flex()
            .flex_col()
            .child(header)
            .child(inner)
            .child(
                div()
                    .flex_none()
                    .h(px(STATUS_HEIGHT))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(TextSize::Xs.font_size())
                    .text_color(muted)
                    .child(div().flex_1().truncate().child(status))
                    .child(Button::new("open-in-editor").label("Open in editor").variant(ButtonVariant::Ghost).size(ButtonSize::Sm).on_click(move |e, w, cx| open_in_editor(e, w, cx))),
            )
            .into_any_element()
    }

    fn failed_or_loading(&self, path: &str) -> String {
        match self.model.entry(path) {
            None => "Reading the files…".into(),
            Some(_) => "Reading the diff…".into(),
        }
    }
}

impl Render for PullView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        // The pane's size after layout; a change that moves the split draws again.
        let this = cx.entity().downgrade();
        let measure = gpui_kit::canvas(
            move |bounds, _, cx| {
                let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                this.update(cx, |s, cx| {
                    if (s.width - width).abs() > 0.5 || (s.height - height).abs() > 0.5 {
                        let before = s.layout();
                        s.width = width;
                        s.height = height;
                        if s.layout() != before || (s.height - height).abs() > 0. {
                            cx.notify();
                        }
                    }
                })
                .ok();
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let height = if self.height > 0. { self.height } else { f32::from(window.viewport_size().height) };
        let handlers = self.handlers(cx);
        let root = |content: gpui_kit::AnyElement| {
            div().relative().flex().size_full().min_w_0().gap(px(RAIL_GAP)).p(px(PADDING / 2.)).bg(theme.background).child(measure).child(content)
        };
        if !self.model.ready() {
            let words = match self.model.error_of(PartKind::Pull) {
                Some(ForgeError::NotSignedIn) => "You are not signed in to GitHub on this host. Run gh auth login there.".to_string(),
                Some(error) => error.to_string(),
                None => "Reading the pull request…".to_string(),
            };
            let retry = cx.listener(|view, _, _, cx| view.retry(cx));
            let failed = self.model.error_of(PartKind::Pull).is_some();
            return handlers.keys(
                root(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(10.))
                        .text_color(theme.muted_foreground)
                        .child(words)
                        .when(failed, |d| d.child(Button::new("retry-pull").label("Try again").variant(ButtonVariant::Secondary).size(ButtonSize::Sm).on_click(move |e, w, cx| retry(e, w, cx))))
                        .into_any_element(),
                ),
                &self.focus,
            );
        }
        self.mark_time("first frame with data");
        let layout = self.layout();
        let progress = self.model.progress();
        let files = self.model.files();
        let seen = self.model.seen_set();
        let current: SharedString = self.model.place.as_ref().filter(|p| !p.brought_in).map(|p| SharedString::from(p.path.clone())).unwrap_or_default();
        let body = (height - 16. - 52.).max(200.);
        let open = cx.listener(|this, path: &SharedString, window, cx| this.open_file(path, window, cx));
        let tree = div().flex_none().w(px(TREE)).h(px(body)).bg(theme.card).rounded(radius::LG).p(px(6.)).child(
            ChangedFileTree::new("pr-tree", files).reviewed(seen).current(current).on_open(move |path, window, cx| open(path, window, cx)),
        );
        let card = self.file_card(body, cx);
        let picker = self.picker_popover(cx);
        let right = div()
            .debug_selector(|| "pr-diff".into())
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(8.))
            .child(ReviewBar::new("pr-bar", progress, handlers.clone()).reviewed_word("seen").mark_label("Seen").next_primary(true).review_mode(self.review_mode))
            .child(div().flex().flex_1().min_w_0().gap(px(8.)).when(layout.tree, |d| d.child(tree)).child(card))
            .children(picker);
        // Under 700 px one part shows at a time, with a switch over it.
        let single = layout.single;
        let rail_height = height - 16. - if single.is_some() { PARTS_HEIGHT } else { 0. };
        let rail = layout.rail.map(|width| self.rail(rail_height, if single.is_some() { width } else { width.min(RAIL_MAX) }, cx));
        let parts = single.map(|part| {
            let this = cx.entity().downgrade();
            div().flex_none().h(px(PARTS_HEIGHT)).flex().items_center().child(
                Segmented::new("pr-parts", [Segment::new("Details").debug_name("pr-part-details"), Segment::new("Files").debug_name("pr-part-files")], usize::from(part == Part::Files))
                    .on_change(move |i, _, cx| {
                        this.update(cx, |view, cx| {
                            view.part = if i == 0 { Part::Details } else { Part::Files };
                            cx.notify();
                        })
                        .ok();
                    }),
            )
        });
        let content = match single {
            Some(Part::Details) => div().flex().flex_1().min_w_0().children(rail).into_any_element(),
            Some(Part::Files) => div().flex().flex_1().min_w_0().child(right).into_any_element(),
            None => div().flex().flex_1().min_w_0().gap(px(RAIL_GAP)).children(rail).child(right).into_any_element(),
        };
        handlers.keys(root(div().flex().flex_col().flex_1().min_w_0().children(parts).child(content).into_any_element()), &self.focus)
    }
}

#[cfg(test)]
mod fold_tests {
    use super::body_needs_fold;

    #[test]
    fn a_short_description_needs_no_fold_button_and_a_long_one_does() {
        assert!(!body_needs_fold("A QA pull request for the lathe pull request view. It adds add, sub and div, and a README line. Safe to close."));
        assert!(body_needs_fold(&"A line of the description.\n".repeat(7)));
        assert!(body_needs_fold(&"word ".repeat(200)));
    }
}
