use atelier_agents::usage_history::Day;
/// Today on this computer's calendar, and its offset from UTC in seconds.
pub fn today() -> (Day, i32) {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as libc::time_t);
    let mut local: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: `secs` and `local` are live for the call, and `localtime_r` writes only into `local`.
    let known = !unsafe { libc::localtime_r(&secs, &mut local) }.is_null();
    let offset = if known { local.tm_gmtoff as i32 } else { 0 };
    (Day::from_epoch_secs(secs, offset), offset)
}
