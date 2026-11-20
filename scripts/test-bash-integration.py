"""Check an existing Bash without reading user startup files or installing tools."""
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
from urllib.parse import quote

integration = Path(__file__).resolve().parent.parent / "shell-integration/bash/rmux.bash"
environment = dict(os.environ, TERM_PROGRAM="rmux", RMUX_SHELL_INTEGRATION="1")
environment.pop("BASH_ENV", None)

def run(commands, env=environment):
    return subprocess.run(["bash", "--noprofile", "--norc", "--noediting", "-i"],
                          input=(commands + "\nexit 0\n").encode(),
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          env=env, timeout=10, check=True).stdout

with tempfile.TemporaryDirectory(prefix="rmux-bash-") as temporary:
    directory = Path(temporary) / "space 世界 %;\x1b"
    directory.mkdir()
    source = f"source {shlex.quote(str(integration))}"
    output = run("\n".join([
        "PS1='fixture> '",
        "PROMPT_COMMAND='printf \"old-hook\\n\" # preserve comment'",
        source, source,
        f"cd -- {shlex.quote(str(directory))}",
        "printf 'HOOK:%s:END\\n' \"$PROMPT_COMMAND\"",
        "PS1='replacement> '",
        "printf 'ready\\n'",
    ]))
    expected = b"\x1b]7;file://localhost" + quote(str(directory), safe="/._~-").encode() + b"\x07"
    assert expected in output, "OSC 7 must encode UTF-8, spaces and controls"
    assert b"old-hook\r\n" in output or b"old-hook\n" in output
    assert b"\n_rmux_bash_precmd:END" in output, "hook must append after a newline"
    assert b"\n_rmux_bash_precmd\n_rmux_bash_precmd:END" not in output
    assert b"\x1b]133;A\x07" in output and b"\x1b]133;B\x07" in output
    assert b"replacement> \x1b]133;B\x07" in output
    assert b"\x1b]133;B\x07\x1b]133;B\x07" not in output
    status_output = run("\n".join([
        "PS1='status> '",
        "PROMPT_COMMAND='printf \"USER-STATUS:%s\\n\" \"$?\"'",
        source,
        "false",
        "(exit 7)",
        "true",
    ]))
    assert b"USER-STATUS:1" in status_output and b"USER-STATUS:7" in status_output
    for status in [0, 1, 7]:
        assert f"\x1b]133;D;{status}\x07".encode() in status_output, "user prompt hooks must not replace command status"
    disabled = run(source, dict(environment, RMUX_SHELL_INTEGRATION="0"))
    assert b"\x1b]133;" not in disabled and b"\x1b]7;" not in disabled
print("bash integration: existing hook, idempotence, prompt replacement, encoded cwd, command status and opt-out passed")
