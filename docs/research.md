# cmux research baseline and evidence

Reviewed for rmux on **November 19, 2026**, using the user's America/Chicago calendar date.

| Baseline | Value |
| --- | --- |
| Upstream repository | [manaflow-ai/cmux](https://github.com/manaflow-ai/cmux) |
| Pinned main revision | [`a76816e4f0788003758b9434ab702068a640fb5d`](https://github.com/manaflow-ai/cmux/tree/a76816e4f0788003758b9434ab702068a640fb5d) |
| Commit timestamp | November 19, 2026, 22:17:27 UTC |
| Latest release returned by GitHub | [v0.65.2.2](https://github.com/manaflow-ai/cmux/releases/tag/v0.65.2.2), published October 27, 2026 |
| Research output | 132 feature families, phased requirements, architecture risks and factual contract inventories |

The recursive repository-tree API response was not truncated. Source material included README, changelog, root documentation guides, public documentation components and English text, configuration schema, checked-in CLI tree, shortcut catalog and capability declarations. Targeted source files were retrieved into a temporary research directory; upstream product code was not copied into rmux.

This is a broad source-backed planning baseline, **not an exhaustive runtime certification**. It does not inspect every implementation branch, open issue, PR, feature flag or dynamically registered method. A command can exist without working on every backend. A proposal can describe functionality that has not shipped. No cmux binary or rmux application was run for this research.

## Evidence hierarchy

Use current runtime behavior at a pinned build for final compatibility decisions. During planning, distinguish release notes, user-facing documentation, implementation declarations and proposals. Do not promote an RFC to a shipped feature, or assume a main-branch document applies to the latest stable release.

The feature matrix uses D/I/B/U/P evidence labels rather than claiming an audited stable-release status for every row. JSON inventory counts describe records, not independent product features. Parent CLI nodes, aliases, schema containers and gated APIs affect the totals.

## Primary source map

All cmux links below are pinned to the research revision.

| Area | Sources | Use in rmux |
| --- | --- | --- |
| Product and release scope | [README](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/README.md), [changelog](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/CHANGELOG.md) | Main capabilities, beta labels, removed behavior and regression scenarios |
| CLI and RPC | [CLI contract](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/cli-contract.md), [command tree](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/cli-command-tree.txt), [capabilities](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/Sources/TerminalController%2BCapabilities.swift) | Command inventory and compatibility requirements |
| Settings and shortcuts | [schema](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/web/data/cmux.schema.json), [shortcuts](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/web/data/cmux-shortcuts.ts), [configuration](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/configuration.md) | Per-key scope and host/terminal mapping |
| Layout | [groups](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/workspace-groups.md), [Dock](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/dock.md), [canvas](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/canvas-layout-design.md) | Identity, movement, focus and lifecycle |
| Terminal | [appearance](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/customizing-appearance.md), [keybindings](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/ghostty-keybindings.md), [fork notes](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/ghostty-fork.md) | Configuration compatibility and rendering/lifecycle gaps |
| Notification and automation | [notifications](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/notifications.md), [events](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/events.md), [rules](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/automations.md) | State, replay and bounded execution |
| Agents | [hook guide](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/agent-hooks.md), [Feed](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/feed.md), [messages](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/agent-messages.md), [Vault](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/vault.md) | Per-provider capability boundaries |
| Browser | [automation page](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/web/app/%5Blocale%5D/%28landing%29/docs/browser-automation/page.tsx), [REPL](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/browser-repl/README.md), [parity report](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/browser-repl/parity-report.md) | Automation operations and backend-specific tests |
| Local persistence | [tmux](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/local-tmux.md), [zellij](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/local-zellij.md) | Detach versus termination versus restore |
| Cloud/device transport | [cloud primitives](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/cloud-agent-primitives.md), [cloud daemon](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/cloud-cmux-tui-daemon.md), [multiple machines](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/multi-machine-sessions.md) | Service-scale roadmap and resource authority |
| Platform tools | [computer use](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/computer-use.md), [Simulator](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/simulator-pane.md), [managed policies](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/managed-device-policies.md) | OS-specific adapters and exceptions |

## Discrepancies requiring follow-up

1. **Stable versus main:** the changelog's latest version heading is 0.65.2 while the release endpoint reports v0.65.2.2. Newer main contains additional functionality. Pin a binary/channel for each future differential test.
2. **Mobile transport/privacy:** English iOS documentation describes direct user-provided networking and notification text transiting push services; newer transport docs describe relay infrastructure, while the changelog reports encrypted notifications/replies. These descriptions cannot establish one current wire/privacy contract. Inspect the selected client/server build before specifying E09–E11.
3. **Agent support:** the hook guide names Hermes in setup/behavior text but omits it from its main integration table, while Feed documents Hermes hooks. Treat each capability independently.
4. **Browser support:** an RPC declaration or legacy port plan does not prove a working operation. The REPL parity report is evidence for its recorded build and cases, not an assurance that all browser commands work in all engines.
5. **Custom actions:** public documentation still labels the action registry nightly, while changelog entries describe related features. Preserve the conservative B label until runtime verification resolves channel availability.
6. **Visual parity:** screenshots are referenced upstream, but exact dimensions, animation timings and all interaction states have not been measured here. P1 needs a visual/interaction specification.
7. **Ghostty parity:** cmux uses a patched engine. Compare required behavior against the selected public libghostty-vt version; neither a Rust rewrite nor the library name guarantees equivalence.

## License and reuse boundary

The pinned repository license declares GPL-3.0-or-later as its default, with separate Business Source License 1.1 terms for the listed web/worker/relay directories. Inspect the applicable file notices before reusing code. The server source is therefore not interchangeable with the desktop code for reuse planning. [Pinned license](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/LICENSE)

This roadmap specifies behavior and records sources; it does not import cmux implementation code or assets, select rmux's license, or establish legal clearance. Decide code reuse and distribution licensing before implementation imports. An independent Rust implementation still needs its own dependency and asset notices.

## Refresh procedure

1. Select an immutable upstream SHA and record the corresponding stable/nightly comparison build.
2. Run `python3 scripts/refresh_cmux_inventory.py --revision <full-sha>` from the repository root. This fetches four source files and regenerates the factual JSON inventories; it does not modify the authored roadmap.
3. Review inventory differences, including removed/renamed entries, aliases, feature gates and configuration defaults. Do not equate additions with shipped features.
4. Read changed guides, changelog and relevant handlers/tests. Add or update feature IDs and acceptance criteria. Update evidence labels and the research baseline together.
5. Preserve unresolved differences, then execute differential scenarios against the selected upstream binary when implementing a feature.

The extractor records SHA-256 hashes for its inputs. A `--source-dir` mode can use a local source snapshot; the caller must ensure that directory matches the requested revision. No upstream code is executed.
