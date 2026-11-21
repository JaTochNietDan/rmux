#!/usr/bin/env python3
"""Isolated native SSH lifecycle fixture. No SSH server or network connection."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def wait_for(check, message):
    deadline = time.monotonic() + 8
    while not check():
        if time.monotonic() >= deadline:
            raise AssertionError(message)
        time.sleep(.02)


def main():
    binary = Path("target/debug/rmux").resolve()
    cli = Path("target/debug/rmuxctl").resolve()
    with tempfile.TemporaryDirectory(prefix="rmux-ssh-smoke-") as directory:
        root = Path(directory)
        endpoint = root / "rpc/rpc.sock"
        session = root / "session.json"
        config = root / "config.json"
        config.write_text(json.dumps({"confirm_close": "never", "session_file": str(session)}))
        fixture = root / "ssh"
        fixture.write_text('#!/bin/sh\ntrap "exit 0" HUP TERM\nprintf "%s\\n" "$@" > "$RMUX_TEST_ROOT/argv"\nprintf "%s" "$$" > "$RMUX_TEST_ROOT/pid"\nprintf "\\033]7;file://localhost/tmp\\007SSH-fixture-ready\\n"\nwhile IFS= read -r line; do printf "fixture-input:%s\\n" "$line"; done\n')
        fixture.chmod(0o700)
        local_shell = root / "local-shell"
        local_shell.write_text('#!/bin/sh\nprintf "%s" "$PWD" > "$RMUX_TEST_ROOT/local-cwd-$$"\n')
        local_shell.chmod(0o700)
        env = {key: value for key, value in os.environ.items() if not key.startswith("RMUX_")}
        # Only the fixture can resolve as ssh. Even a missing fixture cannot
        # fall through to a real SSH client or make a network connection.
        env.update(PATH=str(root), RMUX_CONFIG=str(config), RMUX_SHELL=str(local_shell),
                   RMUX_SOCKET_DIR=str(endpoint.parent), RMUX_TEST_ROOT=str(root))
        target = "ssh://fixture-user@[2001:db8::1]:2222"

        def call(method, params=None, fail=False):
            result = subprocess.run([str(cli), "--socket", str(endpoint), method, json.dumps(params or {})],
                                    capture_output=True, text=True, timeout=10)
            reply = json.loads(result.stdout)
            if fail:
                assert result.returncode == 1 and "error" in reply, reply
            else:
                assert result.returncode == 0 and "result" in reply, (reply, result.stderr)
            return reply.get("result")

        for restored in [False, True]:
            with tempfile.TemporaryFile() as log:
                app = subprocess.Popen([str(binary)], env=env, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.DEVNULL, stderr=log)
                try:
                    wait_for(lambda: endpoint.exists() or app.poll() is not None, "socket startup timed out")
                    assert app.poll() is None, "native app exited during startup"
                    window = call("window.list")["windows"][0]["id"]
                    if not restored:
                        original = call("workspace.list")["workspaces"][0]["id"]
                        result = subprocess.run([str(cli), "--socket", str(endpoint), "new-workspace", "--window", window, "--ssh", target],
                                                capture_output=True, text=True, timeout=10)
                        assert result.returncode == 0, (result.stdout, result.stderr)
                        created = json.loads(result.stdout)["result"]["id"]
                        wait_for(lambda: (root / "argv").exists() and (root / "pid").exists(), "fixture did not start")
                        assert (root / "argv").read_text().splitlines() == ["-l", "fixture-user", "-p", "2222", "--", "2001:db8::1"]
                        surfaces = call("surface.list", {"workspace_id": created})["surfaces"]
                        assert len(surfaces) == 1 and surfaces[0]["ssh_destination"] == target, surfaces
                        assert surfaces[0]["saved_ssh_destination"] is None and surfaces[0]["directory_label"] is None
                        wait_for(lambda: call("surface.list", {"workspace_id": created})["surfaces"][0]["remote_directory"] == "/tmp", "remote directory label did not arrive")
                        assert surfaces[0]["process_id"] == int((root / "pid").read_text())
                        assert [row["id"] for row in call("workspace.list")["workspaces"] if row["selected"]] == [original]
                        original_surface = surfaces[0]
                        result = subprocess.run([str(cli), "--socket", str(endpoint), "split-pane", "--pane", original_surface["pane_id"], "--direction", "right", "--ssh", target],
                                                capture_output=True, text=True, timeout=10)
                        assert result.returncode == 0, (result.stdout, result.stderr)
                        split = json.loads(result.stdout)["result"]
                        surfaces = call("surface.list", {"workspace_id": created})["surfaces"]
                        assert len(surfaces) == 2 and all(row["ssh_destination"] == target for row in surfaces)
                        sibling = next(row for row in surfaces if row["id"] == split["surface_id"])
                        assert sibling["process_id"] != original_surface["process_id"]
                        assert any(row["id"] == original_surface["id"] and row["process_id"] == original_surface["process_id"] for row in surfaces)
                        assert [row["id"] for row in call("workspace.list")["workspaces"] if row["selected"]] == [original]
                        fixture_pids = [row["process_id"] for row in surfaces]
                        local = call("pane.split", {"pane_id": original_surface["pane_id"], "direction": "down", "cwd": str(root)})
                        local_surface = next(row for row in call("surface.list", {"workspace_id": created})["surfaces"] if row["id"] == local["surface_id"])
                        assert local_surface["ssh_destination"] is None and local_surface["saved_ssh_destination"] is None
                        cwd_record = root / f"local-cwd-{local_surface['process_id']}"
                        wait_for(cwd_record.exists, "local shell did not record its cwd")
                        assert Path(cwd_record.read_text()).resolve() == root.resolve()
                        call("pane.split", {"pane_id": original_surface["pane_id"], "direction": "down", "cwd": str(root / "missing-directory")}, fail=True)
                        assert len(call("surface.list", {"workspace_id": created})["surfaces"]) == 3
                        local_workspace = call("workspace.create", {"window_id": window, "cwd": str(root), "name": "Local fixture"})["id"]
                        local_surface = call("surface.list", {"workspace_id": local_workspace})["surfaces"][0]
                        assert local_surface["ssh_destination"] is None
                        cwd_record = root / f"local-cwd-{local_surface['process_id']}"
                        wait_for(cwd_record.exists, "local workspace did not record cwd")
                        assert Path(cwd_record.read_text()).resolve() == root.resolve()
                        assert [row["id"] for row in call("workspace.list")["workspaces"] if row["selected"]] == [original]
                        call("workspace.create", {"window_id": window, "cwd": str(root / "missing-directory")}, fail=True)
                        assert len(call("workspace.list")["workspaces"]) == 3
                        fixture.unlink()
                        call("pane.split", {"pane_id": original_surface["pane_id"], "direction": "down", "ssh_destination": "missing-client"}, fail=True)
                        assert len(call("surface.list", {"workspace_id": created})["surfaces"]) == 3

                        call("workspace.create", {"window_id": window, "ssh_destination": "missing-client"}, fail=True)
                        assert len(call("workspace.list")["workspaces"]) == 3
                    else:
                        surfaces = call("surface.list")["surfaces"]
                        saved = [row for row in surfaces if row["saved_ssh_destination"] == target]
                        assert len(saved) == 2 and all(row["ssh_destination"] is None for row in saved), surfaces
                        assert all(row["title"].startswith("Local · ") for row in saved), saved
                    call("system.quit")
                    app.wait(timeout=8)
                    assert app.returncode == 0
                finally:
                    if app.poll() is None:
                        app.terminate()
                        try:
                            app.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            app.kill()
                            app.wait(timeout=5)
                if not restored:
                    def stopped():
                        for pid in fixture_pids:
                            try:
                                os.kill(pid, 0)
                                return False
                            except ProcessLookupError:
                                pass
                        return True
                    wait_for(stopped, "fixture process survived app shutdown")
                    assert target in session.read_text(), "SSH target missing from recovery"
        print("PASS: native SSH CLI/RPC workspace/remote/local split creation, invalid-cwd rollback, source-process preservation, literal argv, metadata isolation, background selection, failed-spawn rollback, teardown and local recovery. No network or visual validation.")


if __name__ == "__main__":
    main()
