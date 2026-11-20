# Tool management

Manage all development tooling through mise. Declare pinned tool versions in
`mise.toml` and run project commands through `mise exec` or mise tasks.

Do not install tools through Homebrew, direct rustup commands, standalone
installers, or other mechanisms without explicit user approval. Tool installs
managed by mise are authorized. Do not create `rust-toolchain.toml` files that
trigger independent rustup auto-installation.

Preserve pre-existing user toolchains and system tools when changing project
setup.
