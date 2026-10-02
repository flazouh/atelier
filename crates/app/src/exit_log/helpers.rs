#[cfg(unix)]
use std::ffi::c_int;

use super::types::SIGNALS;

/// Puts the hooks in place. Call it once, first thing in `main`.
pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let at = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        eprintln!("exit on a panic{at}");
        default(info);
    }));
    #[cfg(unix)]
    // Safety: the handler only calls async-signal-safe functions (write, signal, raise), and atexit
    // takes a plain function.
    unsafe {
        for signal in SIGNALS {
            libc::signal(signal, on_signal as *const () as libc::sighandler_t);
        }
        libc::atexit(on_exit);
    }
}

/// The reader asked the window to close, and nothing held it open.
pub fn closed() {
    eprintln!("exit: the window closed");
}

/// The reader quit.
pub fn quit() {
    eprintln!("exit: Quit");
}

/// The last window is gone: the app ends.
pub fn last_window_closed() {
    eprintln!("exit: last window closed");
}

/// The line a signal leaves in the log.
#[cfg(unix)]
pub(super) fn words(signal: c_int) -> &'static [u8] {
    match signal {
        libc::SIGTERM => b"exit on SIGTERM: something asked atelier to stop\n",
        libc::SIGINT => b"exit on SIGINT: Ctrl+C where atelier was started\n",
        libc::SIGHUP => b"exit on SIGHUP: the terminal or the session that started atelier went\n",
        libc::SIGQUIT => b"exit on SIGQUIT\n",
        _ => b"exit on a signal\n",
    }
}

#[cfg(unix)]
pub(super) extern "C" fn on_signal(signal: c_int) {
    let line = words(signal);
    // Safety: async-signal-safe calls only. The signal goes on to its default, which ends the process.
    unsafe {
        libc::write(2, line.as_ptr().cast(), line.len());
        libc::signal(signal, libc::SIG_DFL);
        libc::raise(signal);
    }
}

#[cfg(unix)]
pub(super) extern "C" fn on_exit() {
    let line: &[u8] = b"exit: the process ended (after the window closed, or with no signal: a lost display ends it this way)\n";
    // Safety: write on stderr, which stays open until the process ends.
    unsafe {
        libc::write(2, line.as_ptr().cast(), line.len());
    }
}
