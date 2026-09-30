//! The sessions open at quit, opened again at launch: each project once, then its sessions in the
//! order their panels had, and the one in front shown.
use lathe_settings::{Location, OpenSession};

/// The projects to open, each once, in the order their first session was open.
pub fn locations(open: &[OpenSession]) -> Vec<Location> {
    let mut places: Vec<Location> = Vec::new();
    for session in open {
        if !places.contains(&session.location) {
            places.push(session.location.clone());
        }
    }
    places
}

/// The saved sessions of `location`, in order.
pub fn of<'a>(open: &'a [OpenSession], location: &Location) -> Vec<&'a OpenSession> {
    open.iter().filter(|s| s.location == *location).collect()
}

#[cfg(test)]
mod tests;
