//! Whether this app may use the microphone. macOS asks the person once, the first time an app records, and remembers the answer;
//! an app that records without asking first gets silence and no error. So the first press asks out loud, and a refusal is
//! reported as a refusal rather than as an empty recording.

/// What the system says about this app and the microphone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Granted,
    /// Not asked yet: asking shows the system's own question.
    Unasked,
    /// The person said no, or a policy does. Only System Settings can change it.
    Refused,
}

#[cfg(target_os = "macos")]
mod mac {
    use std::{sync::mpsc, time::Duration};

    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

    use super::Access;

    /// How long the system's question may stay up before the press gives up on it.
    const PATIENCE: Duration = Duration::from_secs(120);

    pub fn status() -> Access {
        let Some(audio) = (unsafe { AVMediaTypeAudio }) else { return Access::Granted };
        match unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) } {
            AVAuthorizationStatus::Authorized => Access::Granted,
            AVAuthorizationStatus::NotDetermined => Access::Unasked,
            _ => Access::Refused,
        }
    }

    /// Shows the system's question and waits for the answer.
    pub fn ask() -> Access {
        let Some(audio) = (unsafe { AVMediaTypeAudio }) else { return Access::Granted };
        let (tx, answer) = mpsc::channel();
        let block = RcBlock::new(move |granted: Bool| {
            tx.send(granted.as_bool()).ok();
        });
        unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &block) };
        match answer.recv_timeout(PATIENCE) {
            Ok(true) => Access::Granted,
            Ok(false) => Access::Refused,
            Err(_) => Access::Unasked,
        }
    }
}

/// The system's answer now, without asking.
pub fn status() -> Access {
    #[cfg(target_os = "macos")]
    return mac::status();
    #[cfg(not(target_os = "macos"))]
    Access::Granted
}

/// The system's answer, asking first when it has none. Blocks while the question is up, so call it off the UI thread.
pub fn ensure() -> Access {
    #[cfg(target_os = "macos")]
    return match mac::status() {
        Access::Unasked => mac::ask(),
        other => other,
    };
    #[cfg(not(target_os = "macos"))]
    Access::Granted
}
