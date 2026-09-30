//! The "Motion" story: the components ported from beui.dev's `motion/` and `blocks/`, each in every state it
//! has. `MOTION_PART=<name>` shows one alone, at the top left of the page, so a screenshot of it can be laid
//! beside the web demo's (`~/shots/beui/<name>-compare.png`). Without it, every part is listed.
use gpui_kit::AppContext as _;
use beui::{ActiveTheme, Segment, Segmented, Button, ButtonSize, ButtonVariant, Checkbox, ColorSelector, MultiOption, MultiSelect, BloomMenu, FileUpload, FileUploadEvent, NotificationItem, NotificationStack, RangeSlider, Swatch, Toast, ToastPatch, ToastPosition, ToastStack, ToastStatus};
use gpui_kit::{
    AnyElement, Context, Entity, Hsla, IntoElement, ParentElement, Render, Rgba, SharedString, Styled, Window, div, px,
};

/// The demo's accents, from `color-selector.preview.tsx`, as red, green and blue bytes: they are the
/// user's data, not the UI's colours.
const ACCENTS: [(&str, [u8; 3], &str); 8] = [
    ("blue", [52, 120, 246], "Blue"),
    ("purple", [146, 112, 232], "Purple"),
    ("pink", [230, 106, 164], "Pink"),
    ("red", [229, 86, 86], "Red"),
    ("orange", [237, 145, 65], "Orange"),
    ("amber", [229, 182, 60], "Amber"),
    ("green", [101, 166, 90], "Green"),
    ("teal", [22, 157, 131], "Teal"),
];

/// The demo's teams, from `multi-select.preview.tsx`; the dots are Tailwind's rose, sky, amber, violet, emerald and slate 500.
fn teams() -> Vec<MultiOption> {
    let dot = |r: u8, g: u8, b: u8| Hsla::from(Rgba { r: r as f32 / 255., g: g as f32 / 255., b: b as f32 / 255., a: 1. });
    let product = "Product teams";
    let business = "Business teams";
    vec![
        MultiOption::new("design", "Design").group(product).dot(dot(244, 63, 94)),
        MultiOption::new("engineering", "Engineering").group(product).dot(dot(14, 165, 233)),
        MultiOption::new("product", "Product").group(product).dot(dot(245, 158, 11)),
        MultiOption::new("research", "Research").group(product).dot(dot(139, 92, 246)),
        MultiOption::new("marketing", "Marketing").group(business).dot(dot(16, 185, 129)),
        MultiOption::new("operations", "Operations").group(business).dot(dot(100, 116, 139)),
    ]
}

/// The web preview's queue: one arrived, one on its way, one that failed.
fn initial_uploads() -> Vec<beui::UploadItem> {
    vec![
        beui::UploadItem::new("brand-assets", "brand-assets.zip", 18_400_000).mime("application/zip").progress(100.).status(beui::UploadStatus::Success),
        beui::UploadItem::new("release-video", "release-cut.mov", 84_200_000).mime("video/quicktime").progress(58.).status(beui::UploadStatus::Uploading),
        beui::UploadItem::new("contracts", "vendor-contract.pdf", 2_800_000).mime("application/pdf").progress(32.).status(beui::UploadStatus::Error).error("Connection lost"),
    ]
}

fn accents() -> Vec<Swatch> {
    ACCENTS
        .iter()
        .map(|(value, [r, g, b], label)| {
            let color = Hsla::from(Rgba { r: *r as f32 / 255., g: *g as f32 / 255., b: *b as f32 / 255., a: 1. });
            Swatch::new(*value, color, *label)
        })
        .collect()
}

pub struct MotionStory {
    part: Option<String>,
    segs: [usize; 3],
    swap: usize,
    switches: [bool; 2],
    tabs: [usize; 3],
    email: Entity<gpui_kit::component::input::InputState>,
    teams: Entity<MultiSelect>,
    toasts: Entity<ToastStack>,
    notes: Entity<NotificationStack>,
    uploads: Entity<FileUpload>,
    bloom: Entity<BloomMenu>,
    upload_variant: beui::UploadVariant,
    upload_ticks: Vec<gpui_kit::Task<()>>,
    position: ToastPosition,
    accent: SharedString,
    /// Owned by the "every state" rows below.
    second: SharedString,
    third: SharedString,
    terms: bool,
    updates: bool,
    all: bool,
    level: f32,
    fine: f32,
}

impl MotionStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // `MOTION_PICK=2,133,247` puts a primary colour in force, to set the web demo's blue beside ours.
        if let Ok(pick) = std::env::var("MOTION_PICK") {
            let bytes: Vec<f32> = pick.split(',').filter_map(|n| n.trim().parse::<f32>().ok()).collect();
            if let [r, g, b] = bytes[..] {
                beui::theme::set_pick(Some(Hsla::from(Rgba { r: r / 255., g: g / 255., b: b / 255., a: 1. })), cx);
            }
        }
        let teams = cx.new(|cx| MultiSelect::new("teams", teams(), window, cx).placeholder("Choose teams").empty("No teams found.").with_values(["design", "engineering"]));
        let toasts = cx.new(|_| ToastStack::new("story-toasts").limit(5).default_duration(std::time::Duration::from_millis(3600)));
        let notes = cx.new(|cx| {
            NotificationStack::new(
                "notes",
                vec![
                    NotificationItem::new("import-failed", "Orders import failed")
                        .description("42s · TimeoutError at Step 2")
                        .trailing(Some(beui::IconName::RotateRight), "2", beui::TrailingTone::Warning),
                    NotificationItem::new("sla-breach", "SLA breach").description("2m 11s · Data enrichment"),
                    NotificationItem::new("sync-fixed", "Product sync auto-fixed").description("5m · 404 on GET /products"),
                ],
                cx,
            )
        });
        let uploads = cx.new(|_| FileUpload::new("uploads").variant(beui::UploadVariant::Centered).words("Drop files to upload", "PDF, images, video or zipped assets").max_files(5));
        let initial = initial_uploads();
        uploads.update(cx, |u, cx| u.set_items(initial, cx));
        cx.subscribe(&uploads, |this, _, event: &FileUploadEvent, cx| match event {
            FileUploadEvent::Added(items) => items.iter().for_each(|i| this.tick_upload(i.id.to_string(), cx)),
            FileUploadEvent::Retried(item) => this.tick_upload(item.id.to_string(), cx),
            FileUploadEvent::Removed(_) => {}
        })
        .detach();
        let bloom = cx.new(|cx| BloomMenu::new("bloom", beui::bloom_menu::default_items(), cx));
        let mut story = Self { segs: [0, 1, 1], swap: 0, switches: [true, false], tabs: [0, 0, 0], email: cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).placeholder("you@example.com")), bloom, uploads, upload_variant: beui::UploadVariant::Centered, upload_ticks: Vec::new(), teams, toasts, notes, position: ToastPosition::BottomRight, part: std::env::var("MOTION_PART").ok(), accent: "blue".into(), second: "green".into(), third: "pink".into(), terms: true, updates: false, all: false, level: 40., fine: 2.5 };
        story.tick_upload("release-video".to_string(), cx);
        story
    }

    /// Moves one file's progress on, as the web preview's timer does: 7 to 19 a step, every 520 ms, until it is done.
    fn tick_upload(&mut self, id: String, cx: &mut Context<Self>) {
        let uploads = self.uploads.downgrade();
        let mut seed = beui::motion::now_millis() as u64 ^ (id.len() as u64) << 7;
        self.upload_ticks.push(cx.spawn(async move |_, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(520)).await;
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let jump = 7. + ((seed >> 33) % 1000) as f32 / 1000. * 12.;
                let mut done = true;
                let id = id.clone();
                let ok = uploads.update(cx, |u, cx| {
                    u.update(&id, |item| {
                        if item.status == beui::UploadStatus::Uploading {
                            item.progress = (item.progress + jump).min(100.);
                            if item.progress >= 100. {
                                item.status = beui::UploadStatus::Success;
                            } else {
                                done = false;
                            }
                        }
                    }, cx)
                });
                if ok.is_err() || done {
                    break;
                }
            }
        }));
    }
    fn shows(&self, name: &str) -> bool {
        self.part.as_deref().is_none_or(|p| p == name)
    }
}

fn section(title: &'static str, theme: &beui::Theme, body: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(title))
        .child(body)
        .into_any_element()
}

impl Render for MotionStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let alone = self.part.is_some();
        let mut parts: Vec<AnyElement> = Vec::new();
        if self.shows("color-selector") {
            let this = cx.entity().downgrade();
            let demo = ColorSelector::new("accent", accents()).label("Accent").value(Some(self.accent.clone())).on_change({
                let this = this.clone();
                move |v, _, cx| {
                    let v = v.clone();
                    this.update(cx, |s, cx| {
                        s.accent = v;
                        cx.notify();
                    })
                    .ok();
                }
            });
            if alone {
                parts.push(demo.into_any_element());
            } else {
                parts.push(section("Color selector: the demo", &theme, demo));
                let mut off = accents();
                off[2] = off[2].clone().disabled(true);
                off[5] = off[5].clone().disabled(true);
                let second = ColorSelector::new("second", off).label("Some swatches cannot be chosen").value(Some(self.second.clone())).on_change({
                    let this = this.clone();
                    move |v, _, cx| {
                        let v = v.clone();
                        this.update(cx, |s, cx| {
                            s.second = v;
                            cx.notify();
                        })
                        .ok();
                    }
                });
                parts.push(section("Color selector: disabled swatches", &theme, second));
                parts.push(section(
                    "Color selector: the whole group disabled",
                    &theme,
                    ColorSelector::new("third", accents()).label("Accent").value(Some("pink".into())).disabled(true),
                ));
                parts.push(section(
                    "Color selector: nothing chosen, in a narrow column",
                    &theme,
                    div().w(px(160.)).child(ColorSelector::new("fourth", accents()).label("Accent").on_change({
                        let this = this.clone();
                        move |v, _, cx| {
                            let v = v.clone();
                            this.update(cx, |s, cx| {
                                s.third = v;
                                cx.notify();
                            })
                            .ok();
                        }
                    })),
                ));
            }
        }
        if self.shows("checkbox") {
            let this = cx.entity().downgrade();
            let flip = |field: fn(&mut MotionStory) -> &mut bool| {
                let this = this.clone();
                move |v: bool, _: &mut Window, cx: &mut gpui_kit::App| {
                    this.update(cx, |s, cx| {
                        *field(s) = v;
                        cx.notify();
                    })
                    .ok();
                }
            };
            let demo = div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(Checkbox::new("terms", self.terms).label("Accept terms and conditions").on_change(flip(|s| &mut s.terms)))
                .child(Checkbox::new("updates", self.updates).label("Email me product updates").on_change(flip(|s| &mut s.updates)))
                .child(Checkbox::new("partial", true).indeterminate(true).label("Select all (partial)").on_change(|_, _, _| {}))
                .child(Checkbox::new("off", true).disabled(true).label("Disabled").on_change(|_, _, _| {}));
            if alone {
                parts.push(demo.into_any_element());
            } else {
                parts.push(section("Checkbox: the demo", &theme, demo));
                parts.push(section(
                    "Checkbox: every state",
                    &theme,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(Checkbox::new("s1", false).label("Unchecked").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s2", true).label("Checked").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s3", false).indeterminate(true).label("Partial").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s4", false).disabled(true).label("Unchecked, disabled").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s5", true).disabled(true).label("Checked, disabled").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s6", false).indeterminate(true).disabled(true).label("Partial, disabled").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s7", true).on_change({
                            let this = this.clone();
                            move |v, _, cx| {
                                this.update(cx, |s, cx| {
                                    s.all = v;
                                    cx.notify();
                                })
                                .ok();
                            }
                        })),
                ));
            }
        }
        if self.shows("range-slider") {
            let this = cx.entity().downgrade();
            let set = |field: fn(&mut MotionStory) -> &mut f32| {
                let this = this.clone();
                move |v: f32, _: &mut Window, cx: &mut gpui_kit::App| {
                    this.update(cx, |s, cx| {
                        *field(s) = v;
                        cx.notify();
                    })
                    .ok();
                }
            };
            let demo = div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .w_full()
                .max_w(px(384.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(theme.muted_foreground)
                        .child("Drag the handle")
                        .child(div().text_color(theme.foreground).child(format!("{}", self.level))),
                )
                .child(RangeSlider::new("range", self.level).step(5.).on_change(set(|s| &mut s.level)));
            if alone {
                parts.push(demo.into_any_element());
            } else {
                parts.push(section("Range slider: the demo", &theme, demo));
                parts.push(section(
                    "Range slider: no dots, the whole range, a fine step, disabled, and a step that does not divide the range",
                    &theme,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .w(px(384.))
                        .child(RangeSlider::new("r1", self.level).ticks(false).on_change(set(|s| &mut s.level)))
                        .child(RangeSlider::new("r2", self.fine).range(0., 5.).step(0.5).on_change(set(|s| &mut s.fine)))
                        .child(RangeSlider::new("r3", 70.).step(10.).disabled(true))
                        .child(RangeSlider::new("r4", 10.).range(0., 10.).step(4.).on_change(|_, _, _| {}))
                        .child(RangeSlider::new("r5", 0.).step(10.).on_change(|_, _, _| {})),
                ));
            }
        }
        if self.part.as_deref() == Some("button") {
            // The web's `button-base` primary (crop this one), and the same with a key cap, as the Settings QA found it.
            parts.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(Button::new("continue").label("Continue").trailing_icon(beui::IconName::ArrowForward).variant(ButtonVariant::Primary).size(ButtonSize::Md))
                    .child(Button::new("new-session").label("New session").cap("⌃ N").variant(ButtonVariant::Primary).size(ButtonSize::Md))
                    .into_any_element(),
            );
        }
        if self.shows("toast-stack") {
            let toasts = self.toasts.clone();
            let example = |label: &'static str, toast: Toast, promise: bool| {
                let toasts = toasts.clone();
                Button::new(label).label(label).variant(ButtonVariant::Secondary).size(ButtonSize::Sm).pill(true).on_click(move |_, _, cx| {
                    let id = toasts.update(cx, |s, cx| s.show(toast.clone(), cx));
                    if promise {
                        let toasts = toasts.clone();
                        cx.spawn(async move |cx| {
                            cx.background_executor().timer(std::time::Duration::from_millis(1800)).await;
                            toasts
                                .update(cx, |s, cx| {
                                    s.update(
                                        &id,
                                        ToastPatch {
                                            title: Some("Publish complete".into()),
                                            description: Some(Some("Toast updated in-place from loading to success.".into())),
                                            status: Some(ToastStatus::Success),
                                            duration: Some(std::time::Duration::from_millis(3200)),
                                        },
                                        cx,
                                    )
                                });
                        })
                        .detach();
                    }
                })
            };
            let clear = {
                let toasts = toasts.clone();
                Button::new("toast-clear").label("Clear").variant(ButtonVariant::Ghost).size(ButtonSize::Sm).pill(true).on_click(move |_, _, cx| {
                    toasts.update(cx, |s, cx| s.clear(cx));
                })
            };
            let pills = ToastPosition::ALL.into_iter().map(|position| {
                let (toasts, this) = (toasts.clone(), cx.entity().downgrade());
                Button::new(position.word())
                    .label(position.word())
                    .variant(if self.position == position { ButtonVariant::Invert } else { ButtonVariant::Ghost })
                    .size(ButtonSize::Sm)
                    .pill(true)
                    .on_click(move |_, _, cx| {
                        this.update(cx, |s, cx| {
                            s.position = position;
                            cx.notify();
                        })
                        .ok();
                        toasts.update(cx, |s, cx| {
                            s.set_position(position, cx);
                            s.show(Toast::new("Position changed").status(ToastStatus::Info).description(format!("New toasts open from {}.", position.word())), cx);
                        });
                    })
            });
            let demo = div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(24.))
                .pt(px(60.))
                .child(div().flex().flex_col().items_center().gap(px(8.)).child(div().text_size(px(14.)).font_weight(gpui_kit::FontWeight::MEDIUM).text_color(theme.foreground).child("Open a real toast")).child(div().max_w(px(384.)).text_center().text_size(px(12.)).text_color(theme.muted_foreground).child("Toasts render fixed on the screen. Change position to open a toast from that edge.")))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_center()
                        .gap(px(8.))
                        .child(example("Title only", Toast::new("Saved").status(ToastStatus::Success).sticky(), false))
                        .child(example("Promise", Toast::new("Publishing component").status(ToastStatus::Loading).description("Bundling source, preview, and registry metadata.").sticky(), true))
                        .child(example("Success", Toast::new("Component published").status(ToastStatus::Success).description("Registry endpoint and raw source are available."), false))
                        .child(example("Error", Toast::new("Snapshot failed").status(ToastStatus::Error).description("Retry after the browser target settles."), false))
                        .child(clear),
                )
                .child(div().flex().flex_wrap().justify_center().gap(px(6.)).children(pills));
            parts.push(if alone { demo.into_any_element() } else { section("Toast stack: the demo (toasts open in the corner of the window)", &theme, demo) });
        }
        if self.shows("notification-stack") {
            let demo = div().flex().w_full().justify_center().pt(px(208.)).pb(px(24.)).child(self.notes.clone());
            parts.push(if alone { demo.into_any_element() } else { section("Notification stack: the demo", &theme, demo) });
        }
        if self.shows("file-upload") {
            let uploads = self.uploads.clone();
            let ready = uploads.read(cx).items().iter().filter(|i| i.status == beui::UploadStatus::Success).count();
            let total = uploads.read(cx).items().len();
            let variants = [(beui::UploadVariant::Centered, "Centered"), (beui::UploadVariant::Row, "Row")];
            let switch = {
                let (this, uploads) = (cx.entity().downgrade(), uploads.clone());
                let selected = variants.iter().position(|(v, _)| *v == self.upload_variant).unwrap_or(0);
                Segmented::new("upload-variant", variants.iter().map(|(_, label)| Segment::new(*label)), selected).on_change(move |i, _, cx| {
                    let variant = variants[i].0;
                    this.update(cx, |s, cx| {
                        s.upload_variant = variant;
                        cx.notify();
                    })
                    .ok();
                    uploads.update(cx, |u, cx| {
                        u.set_variant(variant, cx);
                        u.set_words(if variant == beui::UploadVariant::Centered { "Drop files to upload" } else { "Drop release files" }, "PDF, images, video or zipped assets", cx);
                    });
                })
            };
            let reset = {
                let (this, uploads) = (cx.entity().downgrade(), uploads.clone());
                Button::new("upload-reset").icon(beui::IconName::RotateLeft).size(ButtonSize::Icon).pill(true).variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                    let items = initial_uploads();
                    uploads.update(cx, |u, cx| u.set_items(items, cx));
                    this.update(cx, |s, cx| s.tick_upload("release-video".to_string(), cx)).ok();
                })
            };
            let card = div()
                .w(px(448.))
                .rounded(px(32.))
                .border_1()
                .border_color(theme.foreground.opacity(0.08))
                .bg(theme.background)
                .p(px(12.))
                .child(
                    div()
                        .mb(px(12.))
                        .px(px(4.))
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(div().child(div().text_size(px(14.)).line_height(px(20.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).text_color(theme.foreground).child("Upload package")).child(div().text_size(px(12.)).line_height(px(16.)).text_color(theme.muted_foreground).child(format!("{ready} of {total} files ready"))))
                        .child(div().flex().items_center().gap(px(6.)).child(div().flex().child(switch)).child(reset)),
                )
                .child(uploads);
            let demo = div().flex().w_full().justify_center().pt(px(24.)).child(card);
            parts.push(if alone { demo.into_any_element() } else { section("File upload: the demo (drop files on it, or press Browse)", &theme, demo) });
        }
        if self.shows("segmented") {
            let this = cx.entity().downgrade();
            let track = |slot: usize, id: &'static str, names: &[&'static str], caps: bool| {
                let this = this.clone();
                let segments = names.iter().map(|n| Segment::new(*n));
                let track = Segmented::new(id, segments, self.segs[slot]).on_change(move |i, _, cx| {
                    this.update(cx, |s, cx| {
                        s.segs[slot] = i;
                        cx.notify();
                    })
                    .ok();
                });
                if caps { track.cap("⌘\\") } else { track }
            };
            let demo = div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(12.))
                .child(track(0, "seg-a", &["Centered", "Row"], false))
                .child(track(1, "seg-b", &["Light", "Dark", "System"], false))
                .child(track(2, "seg-c", &["Side by side", "Single view"], true));
            parts.push(if alone { demo.into_any_element() } else { section("Segmented", &theme, demo) });
        }
        if self.shows("island") {
            let demo = div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(16.))
                .child(beui::SessionsIsland::new("island-a", beui::IslandCounts { running: 2, needs: 0, done: 0 }))
                .child(beui::SessionsIsland::new("island-b", beui::IslandCounts { running: 2, needs: 1, done: 0 }))
                .child(beui::SessionsIsland::new("island-c", beui::IslandCounts { running: 1, needs: 2, done: 3 }));
            parts.push(if alone { demo.into_any_element() } else { section("Island (sessions)", &theme, demo) });
        }
        if self.shows("action-swap") {
            const WORDS: [&str; 4] = ["Commit", "Push", "Open pull request", "Open #3344"];
            let this = cx.entity().downgrade();
            let demo = div().flex().items_center().gap(px(16.)).child(
                beui::ActionSwapButton::new("swap-demo", WORDS[self.swap % 4])
                    .size(beui::SwapSize::Medium)
                    .cap("⌘↵")
                    .debug_name("swap-demo")
                    .on_click(move |_, _, cx| {
                        this.update(cx, |s, cx| {
                            s.swap += 1;
                            cx.notify();
                        })
                        .ok();
                    }),
            );
            parts.push(if alone { demo.into_any_element() } else { section("Action swap", &theme, demo) });
        }
        if self.shows("animated-badge") {
            use beui::BadgeStatus as S;
            let one = |id: &'static str, status: S, label: &'static str| beui::AnimatedBadge::new(id, status).size(beui::BadgeSize::Small).label(label);
            let demo = div()
                .flex()
                .flex_wrap()
                .items_start()
                .gap(px(8.))
                .w(px(360.))
                .child(one("ab-queued", S::Neutral, "Queued"))
                .child(one("ab-live", S::Info, "Live"))
                .child(one("ab-indexing", S::Loading, "Indexing"))
                .child(one("ab-verified", S::Success, "Verified"))
                .child(one("ab-pending", S::Warning, "Pending"))
                .child(one("ab-blocked", S::Danger, "Blocked"));
            parts.push(if alone { demo.into_any_element() } else { section("Animated badge", &theme, demo) });
        }
        if self.shows("loader") {
            let demo = div().flex().items_center().gap(px(24.)).text_color(theme.foreground)
                .child(beui::spinner::Spinner::new("loader-32").size(px(32.)))
                .child(beui::spinner::Spinner::new("loader-20").size(px(20.)))
                .child(beui::spinner::Spinner::new("loader-12").size(px(12.)));
            parts.push(if alone { demo.into_any_element() } else { section("Loader (spinner)", &theme, demo) });
        }
        if self.shows("switch") {
            let this = cx.entity().downgrade();
            let one = |slot: usize, id: &'static str, label: &'static str| {
                let this = this.clone();
                beui::Switch::new(id, self.switches[slot]).label(label).debug_name(id).on_change(move |on, _, cx| {
                    this.update(cx, |s, cx| {
                        s.switches[slot] = on;
                        cx.notify();
                    })
                    .ok();
                })
            };
            let demo = div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(12.))
                .child(one(0, "switch-on", "Enable notifications"))
                .child(one(1, "switch-off", "Off"))
                .child(beui::Switch::new("switch-disabled", true).label("Disabled").disabled(true).debug_name("switch-disabled"));
            parts.push(if alone { demo.into_any_element() } else { section("Switch", &theme, demo) });
        }
        if self.shows("tabs") {
            let this = cx.entity().downgrade();
            let set = |slot: usize, variant: beui::TabsVariant, id: &'static str, names: [&'static str; 3]| {
                let this = this.clone();
                beui::Tabs::new(id, variant, names.map(|n| beui::Tab::new(n).debug_name(format!("{id}-{n}"))), Some(self.tabs[slot])).on_select(
                    move |i, _, cx| {
                        this.update(cx, |s, cx| {
                            s.tabs[slot] = i;
                            cx.notify();
                        })
                        .ok();
                    },
                )
            };
            let demo = div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(24.))
                .child(set(0, beui::TabsVariant::Pill, "tabs-pill", ["Overview", "Activity", "Settings"]))
                .child(set(1, beui::TabsVariant::Segment, "tabs-segment", ["Day", "Week", "Month"]))
                .child(set(2, beui::TabsVariant::Underline, "tabs-underline", ["All", "Open", "Closed"]));
            parts.push(if alone { demo.into_any_element() } else { section("Tabs", &theme, demo) });
        }
        if self.shows("text-input") {
            let demo = div().w(px(360.)).child(
                beui::TextInput::new("email", &self.email)
                    .label("Email")
                    .left_icon(beui::IconName::Search)
                    .reserve_error_line(true)
                    .surface(theme.background)
                    .debug_name("text-input"),
            );
            parts.push(if alone { demo.into_any_element() } else { section("Text input", &theme, demo) });
        }
        if self.shows("menu") {
            let menu = beui::Menu::new(
                "menu-demo",
                [
                    beui::menu::Entry::Label("Project files".into()),
                    beui::MenuItem::new("Open").icon(beui::IconName::Visibility).shortcut("↵").into(),
                    beui::MenuItem::new("Rename").icon(beui::IconName::Edit).shortcut("R").into(),
                    beui::MenuItem::new("Duplicate").icon(beui::IconName::Copy).shortcut("⌘D").into(),
                    beui::MenuItem::new("Download").icon(beui::IconName::Download).into(),
                    beui::menu::Entry::Separator,
                    beui::MenuItem::new("Keep offline").choice(beui::menu::Choice::Check(false)).close_on_select(false).into(),
                    beui::menu::Entry::Separator,
                    beui::MenuItem::new("Move to trash").icon(beui::IconName::Delete).tone(beui::menu::Tone::Destructive).shortcut("⌘⌫").into(),
                ],
            )
            .min_width(240.)
            .debug_name("menu-demo");
            let demo = div().flex().child(menu);
            parts.push(if alone { demo.into_any_element() } else { section("Menu", &theme, demo) });
        }
        if self.shows("bloom-menu") {
            let demo = div().flex().w_full().min_h(px(420.)).justify_center().pt(px(96.)).items_start().child(self.bloom.clone());
            parts.push(if alone { demo.into_any_element() } else { section("Bloom menu: the demo", &theme, demo) });
        }
        if self.shows("multi-select") {
            let demo = div().w(px(384.)).child(self.teams.clone());
            parts.push(if alone { demo.into_any_element() } else { section("Multi select: the demo", &theme, demo) });
        }
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(28.))
            .p(px(24.))
            .bg(theme.background)
            .children(parts)
            .child(self.toasts.clone())
    }
}
