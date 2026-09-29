//! lathe, the app. `lathe [folder…]` opens the window, with each folder named as a project in it.
//!
//! `LATHE_TIMINGS=1` prints when the first frame showed, counted from the start of the process.

use std::{path::PathBuf, time::Instant};

use gpui_kit::{AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size};

mod editor_pane;
mod open_project;
mod shell;
mod ssh_form;
mod tabs;
mod tree;
mod tree_view;

fn main() {
    let started = Instant::now();
    // Read before the event loop starts, so the UI thread never waits on the disk.
    let saved = lathe_settings::path().map(|p| lathe_settings::load(&p)).unwrap_or_default();
    let folders: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    gpui_kit::application().with_assets(lathe_agents::Assets).run(move |cx| {
        beui::init(cx);
        shell::bind_keys(cx);
        if let Some(theme) = saved.theme.as_deref().and_then(beui::themes::named) {
            beui::theme::set_theme(theme.clone(), cx);
        }
        let (w, h) = std::env::var("LATHE_SIZE")
            .ok()
            .and_then(|v| v.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))))
            .unwrap_or((1280., 820.));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(w), px(h)), cx))),
            titlebar: Some(TitlebarOptions {
                title: Some("lathe".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.), px(12.))),
            }),
            ..Default::default()
        };
        let recent = saved.recent.clone();
        cx.open_window(options, move |window, cx| {
            let shell = cx.new(|cx| shell::Shell::new(recent, cx));
            window.focus(&shell.read(cx).focus_handle(), cx);
            for folder in folders {
                shell.update(cx, |s, cx| s.open_local(folder, window, cx));
            }
            // Closing the window with unsaved edits asks first.
            let asking = shell.downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                let unsaved = asking.upgrade().map_or(0, |s| s.read(cx).unsaved(cx));
                if unsaved == 0 {
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
            if std::env::var("LATHE_TIMINGS").is_ok_and(|v| v == "1") {
                window.on_next_frame(move |_, _| eprintln!("first frame after {:.1} ms", started.elapsed().as_secs_f64() * 1000.));
            }
            // gpui-component inputs need its Root at the top of the window.
            cx.new(|cx| gpui_kit::component::Root::new(shell, window, cx))
        })
        .expect("open the window");
        cx.activate(true);
    });
}
