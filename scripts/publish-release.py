#!/usr/bin/env python3
"""Publish the runner-built archive before advertising it in the update feed."""
import os
import hashlib
from pathlib import Path
import shutil
import subprocess
import tomllib


def run(*args):
    return subprocess.run(args, check=True, text=True)

version = tomllib.loads(Path('Cargo.toml').read_text())['workspace']['package']['version']
tag = 'v' + version
repo = 'rmuxapp/rmux'
output = Path('target/releases')/version
assets = [output / f'rmux-{version}-{suffix}' for suffix in (
    'macos-arm64.zip', 'macos-arm64.dmg', 'linux-x86_64.tar.gz', 'windows-x86_64.zip')]
assets.append(output / 'appcast.xml')
for asset in assets:
    if not asset.is_file() or not asset.stat().st_size:
        raise SystemExit(f'Missing release artifact: {asset}')
checksums = output / 'SHA256SUMS'
with checksums.open('w') as stream:
    for asset in assets:
        with asset.open('rb') as data:
            digest = hashlib.file_digest(data, 'sha256').hexdigest()
        stream.write(f'{digest}  {asset.name}\n')
# Publish only after all native jobs succeed. Never replace a published version.
run('gh', 'release', 'create', tag, *(str(path) for path in assets), str(checksums),
    '--repo', repo, '--target', os.environ['GITHUB_SHA'], '--title', f'rmux {version} — preview',
    '--notes-file', f'docs/releases/{tag}.md', '--prerelease', '--draft')
run('gh', 'release', 'edit', tag, '--repo', repo, '--draft=false')
run('git', 'fetch', 'origin', 'main')
run('git', 'checkout', '-B', 'main', 'origin/main')
Path('updates').mkdir(exist_ok=True)
shutil.copyfile(output/'appcast.xml', 'updates/appcast.xml')
run('git', 'config', 'user.name', 'github-actions[bot]')
run('git', 'config', 'user.email', '41898282+github-actions[bot]@users.noreply.github.com')
run('git', 'add', 'updates/appcast.xml')
run('git', 'commit', '-m', f'Publish Sparkle feed for {tag}')
run('git', 'push', 'origin', 'main')
