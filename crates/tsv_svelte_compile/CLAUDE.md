# tsv_svelte_compile

> Svelte-to-JS compiler (pinned to Svelte's `compile()` as oracle) plus the JavaScript canonicalizer that makes oracle comparison meaningful.

## Architecture Position

Depends on:

- [`tsv_lang`](../tsv_lang/CLAUDE.md) — `ParseError`
- `tsv_svelte` — component parsing (`parse`)
- `tsv_ts` — JavaScript/TypeScript parsing (`parse_with_goal`) and the canonical reprint (`format_canonical`)
- `tsv_css` — embedded CSS (reserved for the code-generation phase)

Oracle: Svelte's own `compile()`. The compiler is measured against it not on raw
output bytes but on the *canonical reprint* of both sides (see the canonicalizer
contract below).

See [../../CLAUDE.md §Project Structure](../../CLAUDE.md#project-structure) for
project-wide conventions.

## Module Map

`src/lib.rs` — the whole crate. Free functions in the tsv pattern:

- `compile(source, &CompileOptions) -> Result<CompileOutput, CompileError>` — a
  walking skeleton: it parses the component (so genuine parse errors surface as
  `CompileError::Parse`) and then returns `CompileError::Codegen`, since code
  generation is not yet implemented.
- `canonicalize_js(source) -> Result<String, CanonicalizeError>` — the
  canonicalizer (below). Lives here because the compiler's own output idempotence
  checks and the oracle comparison both consume it.

Types: `CompileOptions { generate: Generate, dev: bool }` (default: `Server`,
non-dev), `CompileOutput { js, css, warnings }`, `CompileWarning { code, message }`
(minimal for now), and the two error enums.

## The Canonicalizer Contract

`canonicalize_js` parses JavaScript as a strict module (`tsv_ts::Goal::Module`)
and reprints it through `tsv_ts::format_canonical`, which erases newline-derived
*authoring intent*:

- **blank lines are dropped** between statements;
- **expansion heuristics are off** — a construct that fits the print width
  collapses to one line whether or not the source had a newline after its opening
  delimiter; it breaks only when width forces it;
- **comments are preserved** in content and relative order, never dropped or
  merged; only their placement is normalized deterministically (an own-line
  comment may become a trailing comment of the preceding node). A construct
  carrying a `//` line comment before more content stays broken — trailing the
  comment onto a continuing line would swallow that content (inside a template
  interpolation it even makes the output unparseable), so comment presence
  overrides collapse there.

Two guarantees follow. **Idempotence**: canonicalizing an already-canonical string
reproduces it. **Authoring-independence**: two programs that differ only in
incidental whitespace reprint to the same string. Together these make a byte
difference between two canonical forms a genuine code difference — the parity bar
for oracle comparison.

The output is self-validated: `canonicalize_js` reparses its own reprint before
returning and surfaces a rejection as `CanonicalizeError::CorruptOutput` — a
canonicalizer bug is loud, never a silently corrupt comparison string.

Real content is *not* intent and survives verbatim: a newline inside a template
literal, a multi-line string via line continuation, and a mapped type's source
multi-line-ness (a deliberate un-erased residual — see the `format_canonical` seam
notes in `tsv_ts`).

## See Also

- Root [`../../CLAUDE.md`](../../CLAUDE.md) — build, test, and workflow commands
- `tsv_ts` `format_canonical` — the intent-erased reprint entry point this crate drives
