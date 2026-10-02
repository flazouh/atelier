use std::time::Instant;

use super::types::{Action, Input, Key, TAKE_BACK};

/// The key's presses, from what it does.
#[derive(Debug, Default)]
pub struct Tracker {
    /// When the key went down, while it is down and the press stands.
    down_at: Option<Instant>,
    /// A tap left the microphone open; the next press of the key stops it.
    locked: bool,
    /// The press was taken back or ended while the key was still down: its release means nothing.
    ignore_up: bool,
}

impl Tracker {
    pub fn feed(&mut self, input: Input, now: Instant) -> Option<Action> {
        match input {
            Input::Down => {
                if self.locked {
                    self.locked = false;
                    self.ignore_up = true;
                    return Some(Action::Release);
                }
                if self.down_at.is_some() {
                    return None;
                }
                self.ignore_up = false;
                self.down_at = Some(now);
                Some(Action::Press)
            }
            Input::Up => {
                if std::mem::take(&mut self.ignore_up) {
                    return None;
                }
                let at = self.down_at.take()?;
                if now.duration_since(at) < TAKE_BACK {
                    self.locked = true;
                    return None;
                }
                Some(Action::Release)
            }
            Input::Other => {
                let at = self.down_at?;
                if now.duration_since(at) >= TAKE_BACK {
                    return None;
                }
                self.down_at = None;
                self.ignore_up = true;
                Some(Action::Cancel)
            }
            Input::Away => {
                let held = self.down_at.take();
                let locked = std::mem::take(&mut self.locked);
                self.ignore_up = held.is_some();
                match held {
                    Some(at) if now.duration_since(at) < TAKE_BACK => Some(Action::Cancel),
                    Some(_) => Some(Action::Release),
                    None if locked => Some(Action::Release),
                    None => None,
                }
            }
        }
    }
}

/// Hears the key `key` names, asked at every event so a new choice applies at once (`None`: no key), in this app's windows,
/// and hands each change to `on`, for as long as the app runs. Only on macOS; elsewhere it does nothing and says so with
/// `false`.
pub fn listen(key: impl Fn() -> Option<Key> + 'static, on: impl Fn(Input) + 'static) -> bool {
    #[cfg(target_os = "macos")]
    return mac::listen(key, on);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (key, on);
        false
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::{cell::Cell, ptr::NonNull};

    use block2::RcBlock;
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventType};

    use super::super::types::{Input, Key};

    /// `kVK_Function`.
    const FN_CODE: u16 = 63;
    /// `NSEventModifierFlagFunction`, and the device bits that say which Option is down (`NX_DEVICERALTKEYMASK`,
    /// `NX_DEVICELALTKEYMASK`).
    const FN_FLAG: usize = 1 << 23;
    const RIGHT_OPTION_BIT: usize = 0x40;
    const LEFT_OPTION_BIT: usize = 0x20;

    /// Whether `key` is down, from one modifier event, or `None` when this event cannot say. The Option keys' state is read
    /// from the device bits every time, so a missed event is put right by the next; Fn has only its own key's events.
    pub(super) fn is_down(key: Key, code: u16, flags: usize) -> Option<bool> {
        match key {
            Key::Fn => (code == FN_CODE).then_some(flags & FN_FLAG != 0),
            Key::RightOption => Some(flags & RIGHT_OPTION_BIT != 0),
            Key::LeftOption => Some(flags & LEFT_OPTION_BIT != 0),
        }
    }

    pub(super) fn listen(key: impl Fn() -> Option<Key> + 'static, on: impl Fn(Input) + 'static) -> bool {
        let down = Cell::new(false);
        let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            let seen = unsafe { event.as_ref() };
            let kind = seen.r#type();
            let Some(key) = key() else {
                down.set(false);
                return event.as_ptr();
            };
            if kind == NSEventType::FlagsChanged {
                let flags = seen.modifierFlags().0;
                if let Some(now) = is_down(key, seen.keyCode(), flags)
                    && now != down.get()
                {
                    down.set(now);
                    on(if now { Input::Down } else { Input::Up });
                }
            } else {
                on(Input::Other);
            }
            event.as_ptr()
        });
        let mask = NSEventMask::FlagsChanged | NSEventMask::KeyDown | NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown;
        let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) };
        // The monitor lasts as long as the app; AppKit keeps the block while it does.
        std::mem::forget(block);
        let installed = monitor.is_some();
        std::mem::forget(monitor);
        installed
    }
}
