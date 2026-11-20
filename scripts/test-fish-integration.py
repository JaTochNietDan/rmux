"""Hook fixture for an existing fish; no user configuration or installations."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from urllib.parse import unquote

fish = shutil.which("fish")
if fish is None:
    raise SystemExit("fish is not installed; this test does not install tools")
integration = Path(__file__).resolve().parent.parent / "shell-integration/fish/rmux.fish"
subprocess.run([fish, "--no-config", "--no-execute", str(integration)], check=True)


def quote(text):
    return "'" + str(text).replace("\\", "\\\\").replace("'", "\\'") + "'"


with tempfile.TemporaryDirectory(prefix="rmux-fish-") as temporary:
    directory = Path(temporary) / "space 世界 %;\x1b"
    directory.mkdir()
    source = "source " + quote(integration)
    commands = "\n".join([
        "function fish_prompt; printf 'user-prompt'; end",
        "function user_hook --on-event fish_prompt; printf 'user-hook'; end",
        source, source,
        "cd " + quote(directory),
        "emit fish_prompt",
        "fish_prompt",
        "emit fish_preexec",
        "false; emit fish_postexec",
        "emit fish_prompt",
    ])

    def run(enabled):
        env = dict(os.environ, TERM_PROGRAM="rmux", RMUX_SHELL_INTEGRATION=enabled,
                   XDG_CONFIG_HOME=temporary, XDG_DATA_HOME=temporary)
        return subprocess.run([fish, "--no-config", "--interactive", "--command", commands],
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              env=env, check=True, timeout=10).stdout

    output = run("1")
    paths = [part.split(b"\x07", 1)[0] for part in output.split(b"\x1b]7;file://localhost")[1:]]
    assert paths and all(b"\x1b" not in path for path in paths)
    assert any(unquote(path.decode()) == str(directory) for path in paths)
    assert b"user-prompt" in output and b"user-hook" in output
    assert output.count(b"\x1b]133;A\x07") == 2, "re-sourcing must not duplicate hooks"
    assert b"\x1b]133;C\x07" in output
    assert b"\x1b]133;D;1\x07" in output
    disabled = run("0")
    assert b"\x1b]133;" not in disabled and b"\x1b]7;" not in disabled
print("fish hooks: cwd, user functions, idempotence, command status and opt-out passed")
