#!/usr/bin/env python3
"""Native macOS SSH authentication checks against a temporary loopback sshd."""
import getpass
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def wait_for(check, message):
    deadline = time.monotonic() + 8
    while not check():
        if time.monotonic() >= deadline:
            raise AssertionError(message)
        time.sleep(.02)


def stop(process):
    if process is not None and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def main():
    binary = Path("target/debug/rmux").resolve()
    cli = Path("target/debug/rmuxctl").resolve()
    with tempfile.TemporaryDirectory(prefix="rmux-ssh-loopback-") as directory:
        root = Path(directory)
        for name in ["host", "client"]:
            subprocess.run(["/usr/bin/ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(root / name)],
                           check=True, capture_output=True)
        authorized = root / "authorized"
        authorized.write_text((root / "client.pub").read_text())
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        server_config = root / "sshd_config"
        server_config.write_text(f"ListenAddress 127.0.0.1\nPort {port}\nHostKey {root}/host\nPidFile {root}/sshd.pid\nAuthorizedKeysFile {authorized}\nAllowUsers {getpass.getuser()}\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nUsePAM no\nStrictModes no\nForceCommand /usr/bin/printf rmux-loopback-authenticated\n")
        known = root / "known_hosts"
        correct_host = f"[127.0.0.1]:{port} " + (root / "host.pub").read_text()
        known.write_text(correct_host)
        client_config = root / "ssh_config"
        client_config.write_text(f"Host loopback-fixture\n HostName 127.0.0.1\n Port {port}\n User {getpass.getuser()}\n IdentityFile {root}/client\n IdentitiesOnly yes\n BatchMode yes\n StrictHostKeyChecking yes\n UserKnownHostsFile {known}\n GlobalKnownHostsFile /dev/null\n")
        wrapper = root / "ssh"
        wrapper.write_text('#!/bin/sh\nexec /usr/bin/ssh -F "$RMUX_TEST_SSH_CONFIG" "$@"\n')
        wrapper.chmod(0o700)
        config = root / "config.json"
        config.write_text(json.dumps({"confirm_close": "never", "session_file": str(root / "session.json")}))
        endpoint = root / "rpc/rpc.sock"
        env = {key: value for key, value in os.environ.items() if not key.startswith("RMUX_")}
        env.update(PATH=str(root), RMUX_CONFIG=str(config), RMUX_SHELL="/usr/bin/true",
                   RMUX_SOCKET_DIR=str(endpoint.parent), RMUX_TEST_SSH_CONFIG=str(client_config))

        def call(method, **params):
            result = subprocess.run([str(cli), "--socket", str(endpoint), method, json.dumps(params)],
                                    capture_output=True, text=True, timeout=10)
            reply = json.loads(result.stdout)
            assert result.returncode == 0 and "result" in reply, (reply, result.stderr)
            return reply["result"]

        server = app = None
        with (root / "server.log").open("w+") as server_log, (root / "app.log").open("w+") as app_log:
            try:
                server = subprocess.Popen(["/usr/sbin/sshd", "-D", "-e", "-f", str(server_config)],
                                          stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=server_log)
                def listening():
                    assert server.poll() is None, "temporary sshd exited"
                    try:
                        with socket.create_connection(("127.0.0.1", port), timeout=.1):
                            return True
                    except OSError:
                        return False
                wait_for(listening, "loopback sshd did not listen")
                app = subprocess.Popen([str(binary)], env=env, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.DEVNULL, stderr=app_log)
                wait_for(lambda: endpoint.exists() or app.poll() is not None, "native app did not listen")
                assert app.poll() is None, "native app exited"
                window = call("window.list")["windows"][0]["id"]

                def connection(expected):
                    workspace = call("workspace.create", window_id=window, ssh_destination="loopback-fixture")["id"]
                    surface = call("surface.list", workspace_id=workspace)["surfaces"][0]
                    assert surface["ssh_destination"] == "loopback-fixture"
                    wait_for(lambda: expected in call("surface.read_text", surface_id=surface["id"])["text"],
                             "SSH terminal did not report expected authentication outcome")
                    wait_for(lambda: call("surface.list", workspace_id=workspace)["surfaces"][0]["process_exited"],
                             "SSH client did not exit")

                connection("rmux-loopback-authenticated")
                known.write_text(f"[127.0.0.1]:{port} " + (root / "client.pub").read_text())
                connection("REMOTE HOST IDENTIFICATION HAS CHANGED")
                known.write_text(correct_host)
                authorized.write_text("")
                connection("Permission denied")
                server_log.flush()
                server_log.seek(0)
                assert server_log.read().count("Accepted publickey") == 1, "unexpected successful authentication"
                call("system.quit")
                app.wait(timeout=8)
                assert app.returncode == 0
            finally:
                stop(app)
                stop(server)
        print("PASS: native SSH config alias/public-key authentication, changed-host-key rejection and unauthorized-key rejection against isolated loopback sshd. Interactive password prompts and visuals unverified.")


if __name__ == "__main__":
    main()
