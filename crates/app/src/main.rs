//! atelier, the app. `atelier [folder…]` opens the window, with each folder named as a project in it;
//! `ssh://host/path` names a folder on an SSH host (`ssh://hp-agent/~/code/atelier`).
//!
//! `ATELIER_TIMINGS=1` prints when the first frame showed, counted from the start of the process.

use std::path::PathBuf;

use gpui_kit::{AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, size};
use atelier_ui::scale::px;

mod activity;
mod agent_session;
mod project_icons;
mod providers;
mod agents_view;
mod file_glyphs;
mod dirty;
mod editor_pane;
mod exit_log;
#[cfg(test)]
mod fake_agent;
#[cfg(test)]
mod fake_forge;
mod frame_meter;
mod glide;
mod handoff_targets;
mod key_table;
mod list_diff;
mod look_rules;
mod memory;
mod open_project;
mod pull_card;
mod pulls;
mod review_pane;
mod review_state;
mod review_text;
mod right_pane;
mod session_panel;
mod session_title;
mod session_view;
mod settings_pane;
mod ship;
mod shell;
mod sidebar_layout;
mod ssh_form;
mod status;
mod tabs;
#[cfg(test)]
mod test_dirs;
mod timings;
mod tool_density;
mod tasks;
mod tree;
mod tree_view;
mod view_cache;
mod worktrees;

fn main() {
    exit_log::install();
    let started = timings::mark_start();
    if let Err(error) = atelier_project::adopt_old_data(None) {
        eprintln!("could not move the data of the version named lathe: {error}");
    }
    // Read before the event loop starts, so the UI thread never waits on the disk.
    let saved = atelier_settings::path().map(|p| atelier_settings::load(&p)).unwrap_or_default();
    // The reader's pick, else the system's; before any window draws a word.
    atelier_i18n::set_current(saved.language.as_deref().and_then(atelier_i18n::Locale::from_tag).unwrap_or_else(atelier_i18n::system_locale));
    let folders: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let folders: Vec<Opening> = folders.into_iter().map(Opening::from).collect();
    gpui_kit::application().with_assets(atelier_agents::Assets).run(move |cx| {
        atelier_ui::init(cx);
        file_glyphs::install(cx);
        cx.set_global(agent_session::RunPickedSkills(saved.run_picked_skills.unwrap_or(false)));
        cx.set_global(tool_density::ToolDensity::from_key(saved.tool_density.as_deref()));
        cx.set_global(providers::DefaultProvider(providers::Choice::saved(saved.default_provider.as_deref())));
        cx.set_global(providers::ProviderServices::system());
        shell::bind_keys(cx);
        // The reader's primary colour first, so every theme that follows wears it; then the theme; then light, dark
        // or the system's, which keeps the pick.
        atelier_ui::design_preview::init_strength(saved.design_strength); // design preview: remove after Alex picks
        atelier_ui::design_preview::init_elevation(saved.design_elevation); // design preview: remove after Alex picks
        atelier_ui::design_preview::init(saved.design_tabs, cx); // design preview: remove after Alex picks
        atelier_ui::theme::set_pick(saved.primary.map(settings_pane::colour), cx);
        if let Some(theme) = saved.theme.as_deref().and_then(atelier_ui::themes::named) {
            atelier_ui::theme::set_theme(theme.clone(), cx);
        }
        if let Some(mode) = saved.mode.as_deref().and_then(settings_pane::Mode::from_key) {
            mode.apply(cx);
        }
        let (w, h) = std::env::var("ATELIER_SIZE")
            .ok()
            .and_then(|v| v.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))))
            .unwrap_or((1280., 820.));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(w), px(h)), cx))),
            // The narrowest width the layout keeps readable (docs/app.md, "Window widths").
            window_min_size: Some(size(px(640.), px(480.))),
            titlebar: Some(TitlebarOptions {
                title: Some("atelier".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.), px(12.))),
            }),
            ..Default::default()
        };
        cx.open_window(options, move |window, cx| {
            let shell = cx.new(|cx| shell::Shell::new(&saved, cx));
            shell.update(cx, |s, cx| s.listen(window, cx));
            window.focus(&shell.read(cx).focus_handle(), cx);
            // The sessions open at the last quit open again with their projects: all of them when no
            // folder is named, else those of the named folders.
            if !saved.open.is_empty() {
                let (open, front, all) = (saved.open.clone(), saved.front.clone(), folders.is_empty());
                shell.update(cx, |s, cx| s.restore(open, front, all, window, cx));
            }
            for folder in folders {
                shell.update(cx, |s, cx| match folder {
                    Opening::Local(path) => s.open_local(path, window, cx),
                    Opening::Ssh { host, path } => s.open_remote(host, path, window, cx),
                });
            }
            // Closing the window with unsaved edits asks first.
            let asking = shell.downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                let unsaved = asking.upgrade().map_or(0, |s| s.read(cx).unsaved(cx));
                if unsaved == 0 {
                    exit_log::closed();
                    return true;
                }
                let tabs = if unsaved == 1 { "1 tab has".to_string() } else { format!("{unsaved} tabs have") };
                let answer = window.prompt(
                    gpui_kit::PromptLevel::Warning,
                    &format!("{tabs} unsaved changes."),
                    Some("They are lost if you close the window."),
                    &["Close Anyway", "Cancel"],
                    cx,
                );
                let handle = window.window_handle();
                cx.spawn(async move |cx| {
                    if answer.await == Ok(0) {
                        _ = handle.update(cx, |_, window, _| window.remove_window());
                    }
                })
                .detach();
                false
            });
            if std::env::var("ATELIER_TIMINGS").is_ok_and(|v| v == "1") {
                window.on_next_frame(move |_, _| eprintln!("first frame after {:.1} ms", started.elapsed().as_secs_f64() * 1000.));
            }
            // gpui-component inputs need its Root at the top of the window.
            cx.new(|cx| gpui_kit::component::Root::new(shell, window, cx))
        })
        .expect("open the window");
        // The app menu holds Quit, as every desktop app's does; where there is no app menu, this does nothing.
        cx.set_menus([gpui_kit::Menu {
            name: "atelier".into(),
            items: vec![gpui_kit::MenuItem::action("Quit atelier", shell::Quit)],
            disabled: false,
        }]);
        // The last window closing ends the app, and says so: on Linux nothing else would end it.
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                exit_log::last_window_closed();
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}

/// A folder named on the command line.
enum Opening {
    Local(PathBuf),
    Ssh { host: String, path: String },
}

impl From<PathBuf> for Opening {
    /// `ssh://host/path` is a folder on a host: `ssh://hp-agent/~/code` is `~/code` there, and
    /// `ssh://hp-agent/home/alex` is `/home/alex`.
    fn from(arg: PathBuf) -> Self {
        let text = arg.to_string_lossy();
        let Some(rest) = text.strip_prefix("ssh://") else { return Opening::Local(arg) };
        let (host, path) = rest.split_once('/').unwrap_or((rest, "~"));
        let path = if path.starts_with('~') { path.to_string() } else { format!("/{path}") };
        Opening::Ssh { host: host.to_string(), path }
    }
}
