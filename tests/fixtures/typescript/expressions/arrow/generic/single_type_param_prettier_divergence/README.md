# Single type param arrow — trailing comma divergence

A generic arrow with a **single type param that has no constraint** — `<T>` or a
default-only `<T = string>` — formats differently in a Svelte context:

- **Prettier**: forces a trailing comma — `<T,>(x: T) => x`
- **tsv**: keeps it bare — `<T>(x: T) => x`

Prettier forces the comma to keep its output valid as TSX: in `.tsx`, a bare `<T>`
is ambiguous with a JSX element, so prettier's `shouldForceTrailingComma`
(`language-js/print/type-parameters.js`) emits `<T,>` for single-unconstrained
arrow type params unless the file is known to be `.ts`. When formatting a Svelte
`<script lang="ts">` (or a template expression), prettier-plugin-svelte hands the
body to prettier without a `.ts`-looking filepath, so the guard fires.

tsv has no JSX: it never emits TSX, and Svelte's own parser accepts bare `<T>` in
every TS position — `<script>`, template expressions `{...}`, and `{@const}`. The
TSX-disambiguation rationale is therefore vestigial here, so tsv emits the bare,
canonical form. (Multi-param `<T, U>`, constrained `<T extends X>`, and empty `<>`
are unaffected — prettier never forces the comma for those, and tsv matches.)

**Accepted tradeoff**: in a repo that runs both formatters, prettier rewrites tsv's
`<T>` back to `<T,>`, so the two ping-pong on this construct. This was reviewed and
accepted: bare `<T>` is the correct canonical form for a non-JSX formatter.

Reason: **Design choice**. See
[conformance_prettier.md](../../../../../../../docs/conformance_prettier.md) §TypeScript.
