#!/usr/bin/env bash
# Drives a running atelier through its control socket (crates/app/src/control.rs), for QA without a pointer.
# A debug build listens by itself. The socket is $ATELIER_CONTROL when set, else the newest live
# atelier-<pid>.sock in $XDG_RUNTIME_DIR (or /tmp).
#   tools/atelier-ctl.sh state
#   tools/atelier-ctl.sh new_session [agent]     e.g. Cursor, "Claude Code"
#   tools/atelier-ctl.sh send "a message"
#   tools/atelier-ctl.sh limit                  pretend the front session hit its weekly limit
# Each prints the app's JSON answer, one line.
set -euo pipefail
[ $# -ge 1 ] || { sed -n 2,9p "$0"; exit 2; }
exec python3 - "$@" <<'PY'
import glob, json, os, socket, sys
cmd, rest = sys.argv[1], sys.argv[2:]
request = {"cmd": cmd}
if cmd == "new_session" and rest:
    request["agent"] = rest[0]
if cmd == "find" or cmd == "click":
    if rest and not rest[0].isdigit():
        request["name"] = rest[0]
    elif len(rest) == 2:
        request["x"], request["y"] = float(rest[0]), float(rest[1])
if cmd == "view":
    request["name"] = rest[0]
if cmd == "send":
    request["text"] = " ".join(rest)

def connect():
    asked = os.environ.get("ATELIER_CONTROL")
    if asked and asked != "off":
        paths = [asked]
    else:
        run = os.environ.get("XDG_RUNTIME_DIR") or "/tmp"
        paths = sorted(glob.glob(os.path.join(run, "atelier-*.sock")), key=os.path.getmtime, reverse=True)
    for path in paths:
        s = socket.socket(socket.AF_UNIX)
        try:
            s.connect(path)
            return s
        except OSError:
            s.close()
            if not asked:
                os.unlink(path)  # left by an app that is gone
    sys.exit("no atelier is listening; start one with tools/dev-qa.sh start")

s = connect()
s.sendall((json.dumps(request) + "\n").encode())
print(s.makefile().readline().strip())
PY
