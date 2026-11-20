#!/usr/bin/env python3
"""Import CI-only credentials into a temporary keychain, then destroy that keychain."""
import base64
import os
from pathlib import Path
import secrets
import shlex
import subprocess
import tempfile


def run(*args, sensitive=False):
    result = subprocess.run(args, capture_output=sensitive, text=True)
    if result.returncode:
        # Never interpolate command arguments or captured output containing credentials.
        raise SystemExit(f'{Path(args[0]).name} failed (exit {result.returncode})')
    return result


def required(name):
    value = os.environ.get(name, '').strip()
    if not value:
        raise SystemExit('Missing required GitHub Actions secret: ' + name)
    return value


os.umask(0o077)
certificate = base64.b64decode(required('MACOS_CERTIFICATE_P12_BASE64'), validate=True)
password = required('MACOS_CERTIFICATE_PASSWORD')
private_key = required('SPARKLE_PRIVATE_KEY')
notary_key = required('APPLE_NOTARY_KEY_P8')
key_id = required('APPLE_NOTARY_KEY_ID')
issuer = os.environ.get('APPLE_NOTARY_ISSUER_ID', '').strip()
original = shlex.split(run('security', 'list-keychains', '-d', 'user', sensitive=True).stdout)
original_default = shlex.split(run('security', 'default-keychain', '-d', 'user', sensitive=True).stdout)[0]
with tempfile.TemporaryDirectory(prefix='rmux-signing-') as temporary:
    root = Path(temporary)
    p12, p8, ed = root/'identity.p12', root/'notary.p8', root/'sparkle.key'
    p12.write_bytes(certificate)
    p8.write_text(notary_key)
    ed.write_text(private_key)
    keychain = str(root/'release.keychain-db')
    keychain_password = secrets.token_urlsafe(32)
    created = False
    try:
        run('security', 'create-keychain', '-p', keychain_password, keychain, sensitive=True)
        created = True
        run('security', 'set-keychain-settings', '-lut', '21600', keychain, sensitive=True)
        run('security', 'unlock-keychain', '-p', keychain_password, keychain, sensitive=True)
        run('security', 'list-keychains', '-d', 'user', '-s', keychain, *original, sensitive=True)
        run('security', 'default-keychain', '-d', 'user', '-s', keychain, sensitive=True)
        run('security', 'import', str(p12), '-k', keychain, '-P', password, '-T', '/usr/bin/codesign', sensitive=True)
        run('security', 'set-key-partition-list', '-S', 'apple-tool:,apple:,codesign:', '-s', '-k', keychain_password, keychain, sensitive=True)
        run('generate_keys', '--account', 'rmux', '-f', str(ed), sensitive=True)
        args = ['xcrun', 'notarytool', 'store-credentials', 'rmux-ci', '--key', str(p8), '--key-id', key_id, '--keychain', keychain]
        if issuer:
            args += ['--issuer', issuer]
        run(*args, sensitive=True)
        os.environ['RMUX_NOTARY_PROFILE'] = 'rmux-ci'
        os.environ['RMUX_NOTARY_KEYCHAIN'] = keychain
        run('sh', 'scripts/release-macos.sh')
    finally:
        run('security', 'default-keychain', '-d', 'user', '-s', original_default, sensitive=True)
        run('security', 'list-keychains', '-d', 'user', '-s', *original, sensitive=True)
        if created:
            run('security', 'delete-keychain', keychain, sensitive=True)
