use std::time::Duration;

/// How much of a process's stderr is kept: enough for a crash's message and its backtrace.
pub const STDERR_KEEP: usize = 64 * 1024;

/// How long [`Tail::finish`](crate::process::Tail::finish) waits for the stream to end after its process did: a grandchild that
/// kept the stream open must not hold up a wait for long.
pub(super) const FINISH_WAIT: Duration = Duration::from_millis(500);

/// How a watchdog ends a child whose app is gone. The child leads a process group of its own, so the
/// watchdog ends the child's whole group, the helpers the child started too. It asks each second whether
/// the app runs (a zombie, killed but not reaped, does not; a host without `ps` cannot tell a zombie, so
/// there the app runs while it exists) and whether the group still has a process; once the app is gone,
/// it stops the group if it has a process, and kills it when it still has one two seconds later. A group
/// id is not reused while the group has a process, so the kill reaches no other process. It ignores the
/// signals a closing terminal sends the app's whole group, so it outlives them to do its work.
pub(super) const TETHER: &str = r#"(
trap '' HUP INT TERM
up() {
  kill -0 "$1" 2>/dev/null || return 1
  s=$(ps -o stat= -p "$1" 2>/dev/null) || return 0
  case $s in *Z*) return 1 ;; esac
}
while up "$1" && kill -0 "-$2" 2>/dev/null; do sleep 1; done
if ! up "$1" && kill -0 "-$2" 2>/dev/null; then
  kill -TERM "-$2" 2>/dev/null
  sleep 2
  kill -0 "-$2" 2>/dev/null && kill -KILL "-$2" 2>/dev/null
fi
) </dev/null >/dev/null 2>&1 &"#;
