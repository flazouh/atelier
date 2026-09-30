//! Giving freed memory back. glibc keeps what a thread frees in that thread's arena, so the app's resident
//! memory only grows: each open and close of a 200-file review kept about 14 MB. After a big thing closes,
//! [`give_back`] asks glibc to return the free pages, off the UI thread. Elsewhere it does nothing.

/// Returns free heap pages to the system now. It walks the heap, so it can take some milliseconds.
pub fn trim_now() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: malloc_trim only walks glibc's own heap; 0 keeps no padding at the top.
    unsafe {
        libc::malloc_trim(0);
    }
}

/// [`trim_now`] on a thread of its own, so the UI never waits for it.
pub fn give_back() {
    if cfg!(all(target_os = "linux", target_env = "gnu")) {
        std::thread::spawn(trim_now);
    }
}

#[cfg(test)]
mod tests;
