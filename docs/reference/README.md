# cmux contract inventories

These factual inventories supplement the [feature matrix](../feature-parity.md). They are generated from cmux commit `a76816e4f0788003758b9434ab702068a640fb5d`. Counts are extraction records, not counts of distinct user features.

| File | Records | Scope |
| --- | ---: | --- |
| [Feature requirements](features.json) | 132 | Authored feature families, phases, evidence keys, acceptance and initial rmux status |
| [CLI inventory](cmux-cli-inventory.json) | 476 | Every command record in the checked-in command tree, aliases, argument kinds and completion metadata |
| [RPC inventory](cmux-rpc-inventory.json) | 493 | Static base capability declarations plus design-mode additions |
| [Settings inventory](cmux-settings-inventory.json) | 535 | Schema paths, alternatives, wildcard entries, containers and definitions; types/defaults when declared |
| [Shortcut inventory](cmux-shortcut-inventory.json) | 166 | Literal action and category IDs, with source line numbers |
| [Input provenance](cmux-contract-sources.json) | 4 | Source paths, immutable links and SHA-256 hashes |

## Limits that matter for implementation

- The CLI tree includes grouping nodes and internal commands. Some hand-parsed command families have subcommands hidden inside an `arguments` field. Their presence must be expanded from the handler/help before claiming complete CLI compatibility.
- The RPC extraction excludes dynamically appended Simulator methods and DEBUG lists. Runtime feature flags can remove methods from the extracted base. The source list is capability discovery, not permission to call every method.
- The settings count includes structural nodes. `$ref` values point into the upstream schema; it is not a count of independent preferences. Descriptions are deliberately left in the linked source rather than duplicated.
- Shortcut category IDs are retained alongside action IDs. Exact combinations, platform translations and defaults remain in the upstream catalog.
- `features.json` resolves source abbreviations through its `source_paths` map and pinned revision; matching links appear in the feature matrix. It is authored scope, not generated runtime evidence.

## How to turn a record into a compatibility test

For each implemented command or setting, record its feature ID, upstream version, supported platforms, arguments/defaults, response/error format, side effects, focus behavior, permission boundary and a reproducible scenario. Test success, invalid target, stale handle, cancellation, teardown and disabled-capability behavior where relevant.

Keep a separate result for rmux's native interface and any cmux compatibility interface. Commands not yet implemented remain explicit gaps. Do not rename the executable and treat help-output similarity as parity.
