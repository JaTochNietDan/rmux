# macOS releases

Tools are pinned in mise.toml. Run all tasks through mise. The current release
target is Apple Silicon. Public updater metadata lives in release/macos.json;
private keys and notarization credentials never belong in Git.

## GitHub Actions (published releases)

Published binaries are built on GitHub-hosted macOS ARM64 runners. Run the
Release macOS workflow from main. It installs pinned tooling through mise,
builds from Cargo.lock, imports encrypted repository secrets into a temporary
keychain, signs and notarizes, uploads release assets, and then commits the
Sparkle feed to main. Its credentials are confined to the signing step and the
temporary keychain is removed afterward. The initial preview is marked as a
GitHub prerelease; the raw-main Sparkle feed supports these previews explicitly.

Required Actions secrets: MACOS_CERTIFICATE_P12_BASE64,
MACOS_CERTIFICATE_PASSWORD, SPARKLE_PRIVATE_KEY, APPLE_NOTARY_KEY_P8,
APPLE_NOTARY_KEY_ID, and APPLE_NOTARY_ISSUER_ID for a team API key.

## Local packaging checks

Set RMUX_SIGN_IDENTITY to your Developer ID Application certificate name and
RMUX_NOTARY_PROFILE to an existing notarytool Keychain profile, then run:

```sh
mise run release-macos
```

This builds an optimized app, embeds mise-managed Sparkle preserving symlinks,
signs nested code inside-out, submits the archive to Apple, verifies acceptance,
staples the ticket, validates Gatekeeper, then generates the final ZIP and signed
Sparkle enclosure. Artifacts are under target/releases/VERSION. The Sparkle
private key is stored under the Keychain account `rmux`. The profile name used
for notarization is configurable and is independent of the app's identity.

The distribution app uses dev.rmux.app; local cargo/debug bundles retain the
prototype identifier and do not load an updater. Sparkle offers its standard
update-check consent flow; unattended installation is not enabled by default.
The native Check for Updates menu uses the standard updater UI on the main
thread. Linux and Windows builds do not load Sparkle.

## Publish

Publish the versioned ZIP and SHA256SUMS as GitHub release assets first. Only
then copy the generated appcast.xml to updates/appcast.xml on main. This ensures
the feed does not advertise an archive that has not been uploaded. Keep the
published archive immutable. Future version numbers must increase in Cargo.toml.

Before publishing, verify the archive signature, bundled updater startup, native
Check for Updates UI, signing/notarization, and a download of the published asset.
The first release cannot prove an end-to-end upgrade from a previously shipped
rmux version; verify install/relaunch when preparing the next version.

## Key recovery

Back up both the Developer ID certificate with its private key as an encrypted
P12 and the rmux Sparkle private key in a password manager. Preserve the P12
password. A public certificate or public Sparkle key alone cannot sign updates.
The Apple notarization login can be recreated independently. See Sparkle's
[rotation guidance](https://sparkle-project.org/documentation/#rotating-signing-keys)
before changing either signing identity. Never rotate both together without a
planned migration.
