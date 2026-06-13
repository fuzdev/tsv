# tsv changelog

Covers the npm packages published from this repo — `@fuzdev/tsv_format_wasm`,
`@fuzdev/tsv_parse_wasm`, and `@fuzdev/tsv_wasm`. All move together at the
`Cargo.toml [workspace.package]` version. Each `## Unreleased` section must be
non-empty and carry a `<!-- bump: patch|minor|major -->` marker; `deno task publish
--wetrun --bump <level>` requires `<level>` to match it, then stamps the section
(marker removed) into the released version's section and seeds a fresh empty
`## Unreleased` (reset to `bump: patch`) for the next cycle.

## Unreleased
<!-- bump: minor -->

- formatting is now **non-configurable by design** — no config files, CLI flags,
  or runtime options, and none are planned (opinionated like `gofmt` and Black).
  Reverses the earlier "config options will come later" note in the docs. No
  change to formatter output or the published API (`format_*` / `parse_*` / the
  `tsv` bin) — a posture formalization plus removal of the internal config
  plumbing that anticipated future options.
- `parse_*` now rejects malformed Svelte control-flow blocks that the canonical
  Svelte parser also rejects: a closing tag that doesn't match its opening block
  (e.g. `{#if x}…{/each}`) and `{#each}` / `{#key}` / `{#snippet}` with no
  expression or name. These previously parsed without error.

## 0.1.0

- init
- add `@fuzdev/tsv_wasm` — the full tool (format + parse) in one package, with a
  `tsv` bin (`format` + `parse` subcommands mirroring the native CLI's flags and
  exit codes; single-threaded WASM — `--jobs` is accepted and ignored)
- slim `@fuzdev/tsv_parse_wasm` to parse-only (the `format_*` exports and their
  printers move to `@fuzdev/tsv_wasm`; wasm drops from ~2.9 MB to ~1.7 MB raw,
  ~895 KB to ~515 KB gzipped)
