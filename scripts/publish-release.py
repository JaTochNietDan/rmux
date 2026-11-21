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
# Pin the tag to the built commit before creating the release. Passing an old
# target_commitish to the releases API can require workflows:write if main's
# workflows changed during the build, even when the tag already exists.
source_sha = os.environ['GITHUB_SHA']
tag_ref = f'refs/tags/{tag}'
remote_tag = subprocess.run(['git', 'ls-remote', '--exit-code', 'origin', tag_ref],
                            capture_output=True, text=True)
if remote_tag.returncode == 2:
    run('git', 'push', 'origin', f'{source_sha}:{tag_ref}')
elif remote_tag.returncode != 0:
    raise SystemExit(f'Cannot inspect release tag: {remote_tag.stderr}')
run('git', 'fetch', 'origin', tag_ref)
tag_sha = subprocess.check_output(['git', 'rev-parse', 'FETCH_HEAD^{commit}'], text=True).strip()
if tag_sha != source_sha:
    raise SystemExit(f'Release tag {tag} points to {tag_sha}, expected built commit {source_sha}')
run('gh', 'release', 'create', tag, *(str(path) for path in assets), str(checksums),
    '--repo', repo, '--verify-tag', '--title', f'rmux {version} — preview',
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
