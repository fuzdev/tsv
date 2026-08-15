# TypeScript in a component with no `lang="ts"` — a tracked over-acceptance

A component whose only `<script>` carries **no** `lang` attribute, using TypeScript syntax
in every island: type annotations in the script itself (`let x: number`), a generic and
typed snippet head (`{#snippet fn<T>(a: T)}`), a typed `{#each}` binding
(`as item: string`), and `satisfies` / `as` casts in expression tags.

## What each side does

- **Svelte** decides TypeScript **once per document** — `lang="ts"` on any `<script>` sets
  `parser.ts` — and every reader keys on that flag: a plain `<script>` is handed to vanilla
  acorn (which rejects `x: number`), the snippet reader matches `<` only when `parser.ts`,
  the block readers hand a typed binding to plain acorn, and the expression readers reject
  `as` / `satisfies`. So Svelte **rejects** this input at the first island it reaches
  (`Unexpected token` on the script). `expected_svelte.json` holds the parse-failure marker.
- **prettier** (via prettier-plugin-svelte) inherits that verdict and throws — no format
  oracle exists, which is what `prettier_rejects.txt` pins (`Expected token (`, the plugin
  reaching the snippet head first).
- **tsv** parses the whole document with its TypeScript-capable parser regardless of the
  flag, so it **accepts** — `expected_ours.json` — and formats it as a fixed point.

## Why this fixture exists

This is **not** a sanctioned correction: Svelte's verdict is the drop-in target, and a
document with no `ts` flag is JavaScript. tsv's Svelte parser simply does not carry the
document's TypeScript flag today (`component_is_typescript` lives in the wire-JSON convert
layer, so it can shape the AST but not gate the parse), which makes this **one class across
every TS-bearing island**, not a snippet bug — the parser-level twin of the over-acceptance
`tsv_svelte_compile` refuses at the compile level. The fixture is the **ledger entry**: it
pins the over-acceptance so it is visible and reviewable, and it fails the day the parser
starts honoring the flag (Svelte's rejection then matches tsv's, and the fixture converts
to an `input_invalid_*` case). Until then, a plain-`<script>` component's TypeScript is
accepted-and-formatted rather than refused — degraded-but-safe on the robustness bar, since
the output is a faithful reprint of what the author wrote.

See [conformance_svelte.md §TypeScript-mode gating](../../../../../docs/conformance_svelte.md#typescript-mode-gating-tracked-over-acceptance)
and [conformance_prettier_svelte.md §Svelte: prettier inherits Svelte's parse verdict](../../../../../docs/conformance_prettier_svelte.md#svelte-prettier-inherits-sveltes-parse-verdict).
