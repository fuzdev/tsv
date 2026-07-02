# Fixture Validation and Patterns Guide

> Validation rules, pattern selection, troubleshooting, and divergence patterns.
> For step-by-step workflow, see ./fixture_workflow.md.
> For naming conventions, see ./fixture_naming.md.
> For divergence catalogs, see ./conformance_prettier.md and ./conformance_svelte.md.

**Terminology**: `prettier_variant_*` = prettier-stable, our formatter normalizes to input. `variant_*` = both formatters keep stable, NOT input. `divergent_variant_*` = prettier-stable, our formatter rewrites to a distinct third stable form (NOT input, NOT the form).

## Quick Start

- **Create a fixture** — `cargo run -p tsv_debug fixture_init <dir> --content '<code>'` or `--stdin`
- **Validate fixtures** — `deno task fixtures:validate [pattern]`
- **Update expected.json** — `deno task fixtures:update:parsed [pattern]`
- **Compare with prettier** — `cargo run -p tsv_debug compare <file>`
- **Test fails?** — See [Troubleshooting](#troubleshooting)
- **Variant doesn't normalize?** — See [Prettier Variant Discovery](#unformatted-variant-doesnt-normalize-prettier-variant-discovery)
- **Fixture design (pre-impl)** — `deno task fixtures:validate --prettier-only <pattern>` — see [fixture_workflow.md](./fixture_workflow.md#tdd-steps)

---

## Input File Types

**Most fixtures use `input.svelte`** - this tests code embedded in Svelte's `<script>` or `<style>` context, which is the primary use case.

**Use `input.ts` only when a feature genuinely can't be tested in `.svelte`** — two cases:

- **Byte-0 file-level features** — hashbang (`#!/usr/bin/env node`), BOM — must be the first byte, before any `<script>` tag
- **Context-dependent formatting** — constructs prettier formats _differently_ standalone vs. embedded (tsv itself is context-free). Known case: arrow/standalone type parameters (`<T>` in TS, `<T,>` in Svelte — the trailing comma disambiguates from JSX/template syntax). Test the pure-TS form in `.ts`. JSDoc casts are **not** a member: they format identically in `.ts` and `<script lang="ts">` (tsv preserves; prettier's oxc-ts strips), so a cast fixture is kept `.ts` *intentionally* (see the `INTENTIONAL_TS` note below), not out of necessity.

> **TS-only _syntax_ is NOT one of these.** `import x = require(...)`, `export = value`, type
> annotations, decorators, `declare`, etc. parse and format **identically** inside `<script lang="ts">`
> (Svelte wraps the same acorn-typescript parser), so they use `input.svelte`. When unsure, write
> `.svelte` with `lang="ts"` and confirm it parses via `canonical_parse`.
>
> **Don't guess from the directory name — verify.** `deno task fixtures:ts-audit` (or
> `cargo run -p tsv_debug ts_fixture_audit [pattern]`) embeds **every** `.ts` file in a fixture (input
> _and_ variants) in `<script lang="ts">` and reports whether it's _necessary_ as `.ts` (byte-0
> feature, Svelte-parse failure, or formats-differently — checked against both tsv and prettier) or
> _convertible_. Variants matter: a paren divergence can live in an
> `unformatted_*_parens.ts`, not `input.ts`, so an input-only check gives false "convertible"s.
> Caveat: _convertible_ means only that **formatting** is identical in both contexts — it doesn't know
> whether the fixture is `.ts` on purpose to cover the standalone `tsv_ts`/acorn path (whose
> `expected.json` pins a different AST than Svelte's). It's a screen, not a mandate. Fixtures that
> are `.ts` deliberately are listed in the audit's `INTENTIONAL_TS` allowlist and reported as
> _intentional_ rather than _convertible_, so the convertible list stays limited to fixtures that are
> genuinely free to move (e.g. `syntax/comments/jsdoc_type_cast_ts_prettier_divergence` is the
> standalone-TS proof that the JSDoc-cast paren divergence holds in TS contexts). Add an entry there
> when a fixture's `.ts`-ness is load-bearing.

**Use `input.css` only for file-level CSS features** that require byte position 0:

- BOM (byte order mark) handling

**Use `input.svelte.ts` for Svelte rune modules** (`.svelte.ts` / `.svelte.js` files):

- Rune syntax: `$state`, `$derived`, `$effect`, `$inspect`

#### Why `.svelte` is the Default (Canonical Source)

**For TypeScript:** Both paths use the same parser (`@sveltejs/acorn-typescript`). Svelte's parser wraps acorn-typescript internally, so `input.svelte` and `input.ts` validate against the same canonical reference. However, `.svelte` tests the real use case (embedded TypeScript) and validates the full `tsv_svelte` formatter path.

**For CSS:** Both paths use Svelte's `parseCss` as the canonical parser source. However, `.svelte` tests the real use case (embedded CSS in `<style>`) and validates through the prettier-svelte plugin, matching how CSS is actually used in Svelte projects.

Each input type — canonical parser source — prettier validation:

- `.svelte` (TypeScript) — Svelte → acorn-typescript — prettier-svelte plugin
- `.ts` — acorn-typescript directly — prettier TypeScript parser
- `.svelte.ts` — acorn-typescript — prettier-svelte plugin
- `.svelte` (CSS) — Svelte's `parseCss` — prettier-svelte plugin
- `.css` — Svelte's `parseCss` — prettier CSS parser

**Bottom line:** Use `.svelte` unless the feature genuinely can't be tested in a Svelte context (the two cases above).

#### Standalone Fixture Differences

**TypeScript (`input.ts`):**

- Uses acorn+typescript parser for `expected.json`
- Validates formatting with prettier's TypeScript parser (not the prettier-svelte plugin) — F2/F3/F4 and the prettier-side N rules all run
- Variant files use `.ts` extension (`unformatted_*.ts`)

```
tests/fixtures/typescript/syntax/comments/hashbang/
├── input.ts              # #!/usr/bin/env node\nconsole.log("hello");
├── expected.json         # From acorn+typescript parser
└── unformatted_*.ts      # Variants use .ts extension
```

**CSS (`input.css`):**

- Uses Svelte's `parseCss` for `expected.json` (external canonical source)
- Validates formatting with prettier's CSS parser (not the prettier-svelte plugin) — F2/F3/F4 and the prettier-side N rules all run
- Variant files use `.css` extension (`unformatted_*.css`)

```
tests/fixtures/css/tokens/whitespace/bom_prettier_divergence/
├── input.css             # Standalone CSS source
├── expected.json         # From Svelte's parseCss
└── unformatted_ours_*.css  # Variants use .css extension
```

**Fixture minimalism**: Consolidate related patterns into one fixture (3-6 cases). One CSS rule for value tests, multiple rules only for selector/cascade interactions. See ./fixture_naming.md for full conventions.

**When to consolidate** into one fixture:

- Testing variations of the same syntax (e.g., parameter styles, expression types)
- Examples are short and self-documenting
- All examples should normalize/format the same way

**When to keep separate** fixtures:

- Distinct features needing isolated validation
- Examples require different `expected.json` or `output_prettier.svelte`
- Error/edge cases that need dedicated README

### When to Use Divergence Patterns

> **Quick reference:** See [Decision Framework](#decision-framework) for when to adopt prettier vs diverge.

Use `expected_ours.json + expected_svelte.json` or `output_prettier.svelte` ONLY for:

✅ **Permanent, intentional differences:**

- Spec compliance (tsv follows spec, Svelte has documented quirk)
- Intentional improvements (tsv normalizes better than Prettier)
- Documented compatibility trade-offs (see ./conformance_svelte.md)

❌ **NOT for:**

- "tsv has a bug but hasn't fixed it yet"
- "This feature isn't implemented"
- "tsv wants to do this but it's TODO"

#### README.md Files: When to Create

**Simple Rule:**

- ✅ **Required**: `*_prettier_divergence` fixtures (documents the quirk/divergence)
- ✅ **Optional**: Complex features needing non-obvious explanation
- ❌ **Never**: Standard fixtures (code should be self-documenting)
- ❌ **Never**: Bug reports, TODOs, or "not implemented yet"

**Valid README content:**

- Why tsv intentionally differs from Svelte/Prettier (permanent design decision)
- Spec references explaining correct behavior
- Edge case explanations (when fixture alone isn't self-documenting)
- Trade-offs in compatibility (intentional choices)

**Length guideline:** Keep READMEs under 50 lines. Move detailed analysis to ./conformance_svelte.md or ./conformance_prettier.md.

**Back-link to the catalog:** End divergence READMEs with
`See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §<section>.`
(adjust the `../` depth; name the specific catalog row in parens when the
section is long, e.g. ``§Comment relocation (`new` to `(`)``). The
conformance doc links forward to every divergence fixture; the back-link
closes the loop. Older READMEs predate this convention — add the line when
touching one.

**"Not Implemented" ≠ Divergence**: If our formatter doesn't support a feature yet, that's a bug to fix — not a reason to create `output_prettier.svelte`. Create the fixture with prettier's output as `input.svelte`, let the test fail, then fix the formatter.

---

### The `_ours` Naming Convention

The `_ours` suffix means "validated against our implementation only, not external tools":

- **`expected_ours.json`** — our parser's AST (paired with `expected_svelte.json` for the canonical AST)
- **`unformatted_ours_*.*`** — normalizes to input with our formatter only, NOT prettier (only in `_prettier_divergence` dirs)

**When to use:**

- When testing ONLY our implementation (external tool has quirks/bugs preventing meaningful comparison)
- Self-documenting: `_ours` = "only tsv cares about this"
- Consistent convention across parser and formatter testing

**When NOT to use:** When both our tool and the external tool should agree — use standard names (`expected.json`, `unformatted_*.*`).

---

## Pattern Selection

### Choosing Fixture Patterns

#### Core Invariant

**Input file ALWAYS formats to itself (idempotent)**

No exceptions. The input file must be formatted with **prettier** (not our formatter) when our formatter doesn't match prettier yet.

**If the formatter doesn't implement a feature yet:**

- ❌ **DON'T** create input with our formatter's buggy/incomplete output
- ❌ **DON'T** add `output_prettier.svelte` to document "what prettier does"
- ❌ **DON'T** add README explaining "future work" or "not implemented yet"
- ✅ **DO** create input with prettier's output (the correct target behavior)
- ✅ **DO** let the test fail - failing tests reveal bugs and track what needs fixing
- ✅ **DO** fix the formatter to make the test pass

#### Decision Trees

**Parser patterns:**

```
Need to test parser?
├─ Our parser matches Svelte → use expected.json (default)
└─ Intentional difference → use expected_ours.json + expected_svelte.json
```

**Formatter patterns:**

```
Does prettier format input.svelte differently?
├─ NO  → Regular fixture (both formatters agree)
│  └─> input.svelte + unformatted_*.svelte
│
└─ YES → DIVERGENCE exists!
   ├─ Step 1: Add _prettier_divergence suffix to directory name
   └─ Step 2: Does prettier have multiple stable outputs?
      ├─ NO  → Simple divergence
      │  └─> input.svelte + output_prettier.svelte + unformatted_ours_*.svelte
      │
      ├─ YES, ours normalizes to input → Quirky divergence
      │  └─> input.svelte + prettier_variant_*.svelte + unformatted_ours_*.svelte
      │      (+ optional output_prettier.svelte when prettier's primary output differs from all quirks)
      │
      ├─ YES, ours also keeps stable → Dual-stable divergence
      │  └─> input.svelte + variant_*.svelte + unformatted_ours_*.svelte
      │      (+ optional output_prettier.svelte when prettier's primary output differs from all stables)
      │
      └─ YES, ours rewrites to a THIRD stable form → Divergent-variant
         └─> input.svelte + divergent_variant_*.svelte + unformatted_ours_*.svelte
             (+ optional output_prettier.svelte when prettier's primary output differs)

Special case: prettier NEVER converges (every pass changes the output — no fixed point)
   └─> input.* + prettier_nonconvergent.txt + README.md, no prettier-claim files (F5/S18)

Special case: prettier THROWS on the input (parse rejection or printer crash — no oracle)
   └─> input.* + prettier_rejects.txt + README.md, no prettier-claim files (F6/S19)

Note: in _prettier_divergence dirs, use unformatted_ours_*.svelte when only our formatter
normalizes to input; plain unformatted_*.svelte is allowed (and N3-validated) when the dir
has no output_prettier.* and prettier also normalizes the variant to input
Tip: Use `deno task fixtures:audit <pattern>` to classify novel prettier outputs
```

#### Pattern Usage Summary

- `expected.json` — Default - our parser matches Svelte
- `expected_ours.json` + `expected_svelte.json` — **Intentional, permanent** parser differences (NOT implementation gaps)
- `output_prettier.svelte` — **Intentional, permanent** formatter differences (**NEVER** "not implemented" - that's a bug to fix!)
- `prettier_variant_*.svelte` — Prettier-stable, our formatter normalizes to input
- `variant_*.svelte` — Both formatters keep stable, NOT normalized to input
- `divergent_variant_*.svelte` — Prettier-stable, our formatter rewrites to a distinct third stable form (NOT input, NOT the form)
- `prettier_intermediate_*.svelte` — Prettier's unstable first-pass output (from `unformatted_ours_*`, converges to input)
- `prettier_intermediate_to_variant_*.svelte` — Prettier's unstable first-pass output (from `unformatted_ours_*`, converges to a `variant_*`/`prettier_variant_*`)
- `audit_signature.txt` — Pins prettier's multi-pass chain from `output_prettier.*` to fixed point (auto-generated; see F4)
- `prettier_nonconvergent.txt` — Prettier never reaches a fixed point on input (no oracle exists); claim live-verified (see F5/S18)
- `prettier_rejects.txt` — Prettier throws on the input (parse rejection or printer crash; no oracle exists); the file's trimmed content is the expected-error substring, claim live-verified (see F6/S19)
- `unformatted_*.svelte` — Normalization tests - both formatters normalize to `input.svelte`
- `unformatted_ours_*.svelte` — Normalization tests - only our formatter normalizes to `input.svelte`
- `unformatted_prettier_*.svelte` — Normalization tests - prettier normalizes to `output_prettier.svelte`
- `input_invalid_*` — Invalid syntax that must fail to parse (test rejection)

#### Example: Hug mode formatting

```
fixture/
├── input.svelte               # <Comp\n\t><div>a</div></Comp\n>  (hug mode - canonical)
├── expected.json              # AST from hug mode input
└── unformatted_compact.svelte # <Comp><div>a</div></Comp>  (normalizes to input)
```

#### Example: Section ordering (after refactor)

```
ordering/
├── input.svelte                                    # Correct order (canonical)
├── expected.json                                   # AST with correct order
├── unformatted_1_instance_module_style.svelte      # Wrong order (normalizes)
├── unformatted_2_instance_style_module.svelte      # Wrong order (normalizes)
└── ... (more unformatted variants)
```

All fixtures use `input.svelte` as canonical source.

---

## Validation Rules

### Fixture Validation (Automatic)

`deno task fixtures:validate` validates fixture correctness.

### All Validation Rules

**Discovery validations (W)** - Directory hierarchy checks (during walk):

- **W1**: Each directory must have EXACTLY ONE of: subdirectories OR an input file
  - Directories with an input file (and no subdirs) are **fixtures** (leaf nodes)
  - Directories with subdirectories (and no input file) are **containers** (intermediate nodes)
  - A directory cannot have both (mixed)
  - A directory cannot have neither (orphan)

**Structural validations (S)** - File structure checks:

1. **S1**: Input file exists (`input.svelte`, `input.ts`, `input.css`, or `input.svelte.ts`)
2. **S2**: Input has correct extension (`.svelte` preferred, `.ts`/`.css` for file-level features, `.svelte.ts` for runes)
3. **S3**: `expected.json` OR (`expected_ours.json` + `expected_svelte.json`) exists
4. **S4**: `expected.json` cannot coexist with `expected_*.json` files
5. **S5**: Both `expected_ours.json` + `expected_svelte.json` exist (if either exists)
6. **S6**: `unformatted_*` has same extension as input file
7. **S7**: `prettier_variant_*` has same extension as input file
8. **S8**: Directory name ends with `_prettier_divergence` when ANY of these exist:
   - `output_prettier.*` (prettier formats input differently), OR
   - `prettier_variant_*.*` files (prettier has quirks), OR
   - `variant_*.*` files (dual-stable forms), OR
   - `divergent_variant_*.*` files (divergent-variant forms), OR
   - `unformatted_ours_*.*` files (testing our formatter only), OR
   - `prettier_intermediate_*.*` or `prettier_intermediate_to_variant_*.*` files (multi-pass convergence), OR
   - `prettier_nonconvergent.txt` (prettier never reaches a fixed point), OR
   - `prettier_rejects.txt` (prettier throws on the input)
9. **S8-rev**: `_prettier_divergence` directories MUST document the divergence with one of:
   - `output_prettier.*` (shows what prettier produces), OR
   - `prettier_variant_*.*` files (shows prettier's stable variants), OR
   - `variant_*.*` files (shows dual-stable forms), OR
   - `divergent_variant_*.*` files (shows divergent-variant forms), OR
   - `unformatted_ours_*.*` + README.md (for normalization divergence), OR
   - `prettier_nonconvergent.txt` + README.md (prettier has no fixed point — see F5), OR
   - `prettier_rejects.txt` + README.md (prettier throws on the input — see F6)
10. **S9**: `_prettier_divergence` directories with `output_prettier.*` CANNOT have `unformatted_*.*` files (use `unformatted_prettier_*` or `unformatted_ours_*`). Without `output_prettier.*`, input is prettier-stable (F3), so `unformatted_*` is allowed and N3 validates it — a `prettier_variant_*`-style divergence dir can still hold variants both formatters normalize to input
11. **S10**: `prettier_variant_*.*` files MUST be in `_prettier_divergence` directories (enforced by S8)
12. **S11**: `unformatted_ours_*.*` files MUST be in `_prettier_divergence` directories (enforced by S8)
13. **S12**: `_svelte_divergence` or `_svelte_prettier_divergence` suffix required when `expected_ours.json`/`expected_svelte.json` exist
14. **S18**: `prettier_nonconvergent.txt` CANNOT coexist with prettier-claim files (`output_prettier.*`, `unformatted_*`, `unformatted_prettier_*`, `prettier_variant_*`, `variant_*`, `divergent_variant_*`, `prettier_intermediate_*`) — prettier has no fixed point, so no prettier-anchored claim is expressible. `unformatted_ours_*` stays allowed (it claims only our formatter's normalization)
15. **S19**: `prettier_rejects.txt` follows the same claim-file rules as S18 (prettier throws, so no prettier-anchored claim is expressible; `unformatted_ours_*` stays allowed) AND is mutually exclusive with `prettier_nonconvergent.txt` (prettier either throws or oscillates, never both)

**TypeScript fixture rules** (`input.ts`):

- Uses acorn+typescript parser for `expected.json` (not Svelte's parser)
- Uses prettier's TypeScript parser for formatting validation
- Variant files use `.ts` extension (`unformatted_*.ts`, `prettier_variant_*.ts`)

**CSS fixture rules** (`input.css`):

- Uses Svelte's `parseCss` for `expected.json` (external canonical source)
- Uses prettier's CSS parser for formatting validation
- Variant files use `.css` extension (`unformatted_*.css`, `prettier_variant_*.css`)

**Svelte rune module rules** (`input.svelte.ts`):

- Uses acorn+typescript parser for `expected.json` (runes are just function calls)
- Uses prettier-svelte for formatting validation
- Variant files use `.svelte.ts` extension

**Content validations (C)** - Meaningful test data:

- **C1**: `unformatted_*` differs from input file
- **C2**: `output_prettier.*` differs from input (if exists)
- **C3**: `prettier_variant_*.*` differs from input
- **C3b**: `variant_*.*` differs from input and from all `prettier_variant_*.*` files
- **C3c**: `divergent_variant_*.*` differs from input and from all `prettier_variant_*.*` and `variant_*.*` files
- **C4**: No duplicate `unformatted_*` within same fixture
- **C5**: No duplicate `prettier_variant_*.*` within same fixture
- **C5b**: No duplicate `variant_*.*` within same fixture
- **C5c**: No duplicate `divergent_variant_*.*` within same fixture
- **C6**: No duplicate input files across fixtures (informational)

**Documentation validations (D)**:

- **D1**: README.md required when divergence artifacts exist (`expected_ours.json`+`expected_svelte.json`, `output_prettier.*`, `prettier_variant_*`, `variant_*`, `divergent_variant_*`, or `prettier_intermediate_*`); optional extra documentation for other complex fixtures

**Parser validations (P)** - Expected ASTs match parser outputs:

- **P1**: `expected.json` matches Svelte parser output
- **P2**: `expected_ours.json` matches our parser output
- **P3**: `expected_svelte.json` matches Svelte parser output
- **P4**: `convert_ast_json_string` output is byte-identical to the `Value` path (`to_string(convert_ast_json)`) on the input file — guards the compact-wire path's typed comment attach and typed offset translation, which expected.json comparisons (always via `convert_ast_json`) don't exercise
- **P5**: typed-walk parity probes — the same byte-identity check on synthesized probe content, so the byte→char machinery (TS's fused char-space conversion via `LocationMapper`; the `translate_typed.rs` walks of `tsv_svelte`/`tsv_css` — plus `tsv_ts`'s, which Svelte's hybrid walk delegates typed islands to — each enumerating struct fields manually; and `tsv_svelte`'s `attach_typed.rs`, which mirrors the `Value` attach dispatcher's comment windows) is exercised on every fixture's AST shapes, not just the few inputs whose own content triggers them. `.ts`/`.svelte.ts`/`.css` inputs get a multibyte variant (prepended multibyte comment shifts all downstream offsets; byte-0 fixtures — hashbang, BOM — are skipped); `.svelte` inputs get a whole-file multibyte variant (prepended HTML comment), a template-comment variant (appended expression tag carrying multibyte comments, driving the island-scoped attach against the `Value` dispatcher), and their `<script>` contents extracted and run as standalone TS — as-is when multibyte plus a synthesized variant. A probe that fails to parse is an error, not a skip

P1–P3 anchor fixtures to the canonical parsers; P4/P5 prove tsv's two
materialization paths agree with each other. A bug shared by BOTH paths is
invisible to all of them — the corpus-scale external oracle for that class is
`deno task corpus:compare:parse` (../benches/js/CLAUDE.md §Parse
Comparison), which deep-diffs the shipped wire against the canonical parsers
on real codebases.

**Formatter validations (F)** - Output correctness:

- **F1**: Input file formats to itself with our formatter (idempotency invariant)
- **F2**: `output_prettier.*` matches prettier's current output
- **F3**: `prettier(input)` equals `input` when no `output_prettier.*` exists (applies to ALL directories including `_prettier_divergence`)
- **F4**: `audit_signature.txt`, when present, byte-matches the live prettier chain from `output_prettier.*` to its fixed point. Pins multi-pass non-idempotent behavior so the audit doesn't flag it as novel, and catches pass-2+ drift that F2 alone (pass-1 only) would miss
- **F5**: when `prettier_nonconvergent.txt` exists, F2/F3/F4 and the prettier-side N rules are replaced by a live check of the claim: `prettier(input) != input` AND `prettier²(input) != prettier(input)`. If prettier converges (either check fails), validation fails with a hint to delete the marker and re-document the divergence normally
- **F6**: when `prettier_rejects.txt` exists, F2/F3/F4 and the prettier-side N rules are replaced by a live check of the claim: `prettier(input)` must return an error whose message contains the marker's trimmed content (the position-stripped error substring). If prettier accepts the input (bug fixed) or throws a different message (bug morphed), validation fails with a hint to re-document or update the marker

**Normalization validations (N)** - Variants normalize correctly:

- **N1**: `prettier_variant_*.*`: `prettier(file) == file` (prettier idempotent, Rule 1)
- **N2**: `prettier_variant_*.*` normalizes to input with our formatter (Rule 2)
- **N3**: `unformatted_*.*` normalizes with prettier (runs wherever `unformatted_*` files are allowed — S9 restricts them to directories where input is prettier-stable)
- **N4**: `unformatted_*.*` normalizes to input with our formatter
- **N5**: `unformatted_ours_*.*` normalizes to input with our formatter (only in `_prettier_divergence` dirs)
- **N6**: `unformatted_ours_*.*`: `prettier(file) != input` (verifies prettier does NOT normalize to input — ensures the `_ours` designation is correct)
- **N7**: `prettier_intermediate_*.*` captures prettier's unstable first-pass output:
  - `prettier(unformatted_ours_X) == prettier_intermediate_X` (matches first-pass output)
  - `prettier(prettier_intermediate_X) != prettier_intermediate_X` (verifies it's unstable)
  - `prettier(prettier_intermediate_X) == input` (converges to stable form)
- **N7b**: `prettier_intermediate_to_variant_*.*` captures prettier's unstable first-pass output when it converges to a documented variant instead of `input`:
  - `prettier(unformatted_ours_X) == prettier_intermediate_to_variant_X` (matches first-pass output)
  - `prettier(prettier_intermediate_to_variant_X) != prettier_intermediate_to_variant_X` (verifies it's unstable)
  - `prettier(prettier_intermediate_to_variant_X) ∈ {variant_*, prettier_variant_*}` (converges to a documented variant, not input)
- **N8**: `unformatted_prettier_*.*`: `prettier(file) == output_prettier.*` (prettier normalizes to its canonical output)
  - Requires `output_prettier.*` to exist
  - Tests that prettier normalizes these variants to prettier's stable output
- **N9**: `variant_*.*` dual-stable variant validation:
  - **N9a**: `prettier(file) == file` (prettier idempotent)
  - **N9b**: `ours(file) == file` (our formatter keeps it verbatim — true dual-stability, not merely reaching *a* fixed point)
  - **N9c**: `ours(file) != input` (must NOT normalize to input — else should be `prettier_variant_*`)
- **N11**: `divergent_variant_*.*` divergent-variant validation (prettier keeps `V`; ours rewrites `V` to a third stable form):
  - **N11a**: `prettier(file) == file` (prettier idempotent — prettier phase)
  - **N11b**: `ours(file) != input` (else should be `prettier_variant_*`)
  - **N11c**: `ours(file) != file` (else it's dual-stable — should be `variant_*`)
  - **N11d**: `ours(ours(file)) == ours(file)` (the rewritten third form is itself a fixed point)
- **N10**: Cross-path discovery — pin Prettier's output of every `unformatted_ours_*`:
  - After N7, unclaimed Prettier outputs from `unformatted_ours_*` (those not == input and not consumed by a `prettier_intermediate*_*`) are checked against the fixture's documented stable forms (`output_prettier.*`, `prettier_variant_*.*`, `variant_*.*`)
  - **Blocking** when the fixture documents stable forms but the output matches none of them — `ValidationError::UndocumentedPrettierOutput`. This means Prettier drifted, or the target is undocumented; add/update a matching `variant_*`/`prettier_variant_*`/`divergent_variant_*` (or a `prettier_intermediate*_*` for multi-pass). This is what pins Prettier's _specific_ one-pass-stable output for a normalization divergence (the analogue of N8 for `output_prettier` and N7b for multi-pass convergence).
  - **Informational** only when the fixture documents the divergence by README alone (no `output_prettier`/`prettier_variant_*`/`variant_*` files): novel outputs are NOTEs suggesting investigation via `deno task fixtures:audit`

**Invalid syntax validations (I)** - Syntax rejection tests:

- **I1**: `input_invalid_*.svelte` must fail to parse with BOTH our parser AND Svelte's parser
- **I2**: `input_invalid_*.ts` must fail to parse with BOTH our parser AND acorn-typescript
- **I3**: `input_invalid_*.css` must fail to parse with BOTH our CSS parser AND Svelte's `parseCss`

Validation failures include detailed error messages and fix instructions.

---

## Common Pitfalls

1. **Stale fixtures** → Run `deno task fixtures:update` before debugging (stale files cause false failures)
2. **Mixing numbered/unnumbered names** → Use `Comp1`, `Comp2` not `Comp`, `Comp2` (see ./fixture_naming.md)
3. **`unformatted_*` that prettier preserves** → `prettier_variant_*` or `variant_*` — run `fixtures:audit` to classify
4. **`unformatted_*` in `_prettier_divergence` dirs** → Use `unformatted_ours_*.svelte` instead
5. **Using divergence patterns for temporary gaps** → Only for permanent, intentional differences (see [fixture_workflow.md Golden Rules](./fixture_workflow.md#golden-rules))
6. **Input file AND subdirectories in same dir** → Move input file to a subdirectory (e.g., `overview/`) or move subdirectories elsewhere
7. **Orphan directory (no input, no subdirs)** → Add an input file or delete the directory
8. **"Prettier-stable" by crash, not by design** → prettier-plugin-svelte
   silently emits the **whole `<script>` verbatim** when the embedded
   formatter throws on a form its `babel-ts` parser rejects (e.g.
   `@(f()).g`, a babel SyntaxError). The fixture pipeline disarms this: the
   sidecar sets `PRETTIER_DEBUG=1`, so the plugin rethrows and
   `fixture_init` / validation report a hard prettier error instead of
   letting a never-actually-formatted input pass F3 byte-identically. If you
   hit such an error, the construct is prettier-unformattable in `.svelte` —
   cover it parser-only or as pure `.ts`. (Forms that only crash prettier's
   `typescript` parser, e.g. `@(a?.b)()`, format normally in `.svelte` and
   fail visibly as pure `.ts`.) The corpus pipeline sets the same env
   (`corpus:compare:format:run`), so fallback forms surface as errors there
   too; the fallback only survives outside the repo tooling (bare prettier
   invocations) — see the triage caveat in
   [conformance_prettier.md](./conformance_prettier.md).

---

## Troubleshooting

### Quick Decision Tree

```
Test failing?
│
├─ Parser (expected.json mismatch)
│  ├─ Stale? → deno task fixtures:update:parsed
│  └─ Real diff? → Compare: diff expected.json <(cargo run -p tsv_cli parse input.svelte --pretty)
│
├─ Formatter (not idempotent / differs from prettier)
│  ├─ Stale? → deno task fixtures:update:formatted
│  ├─ Real diff? → cargo run -p tsv_debug compare input.svelte
│  ├─ Stale prettier_nonconvergent.txt (F5)? → prettier converges now; delete the
│  │  marker and document normally (the error's hint names which check failed)
│  └─ Stale prettier_rejects.txt (F6)? → prettier accepts now (delete marker,
│     document normally) or throws a different message (update the marker substring)
│
└─ Normalization (unformatted_* doesn't normalize)
   └─ Check prettier: cargo run -p tsv_debug format_prettier unformatted_X.svelte | diff - input.svelte
      ├─ Prettier preserves → Use `deno task fixtures:audit` to classify
      │  ├─ Ours normalizes to input → prettier_variant_*
      │  ├─ Ours keeps stable (not input) → variant_*
      │  └─ Ours rewrites to a third stable form (not input, not the form) → divergent_variant_*
      └─ Prettier normalizes → fix our formatter
```

### When Tests Fail - Troubleshooting Procedures

#### 1. Understand the Fixture

Read the fixture content to understand what it's testing:

```bash
cat tests/fixtures/css/at_rules/container_spacing_prettier_divergence/input.svelte
```

Fixture names and content should be self-explanatory. Check for README.md in complex fixtures (escapes, entities, spec edge cases).

#### 2. Compare Outputs

```bash
# Visual diff with prettier (shows formatting differences)
cargo run -p tsv_debug compare tests/fixtures/svelte/elements/block_siblings/input.svelte

# AST comparison (semantic equivalence check)
cargo run -p tsv_debug ast_diff tests/fixtures/svelte/elements/block_siblings/input.svelte
```

#### 3. Inspect ASTs (If Needed)

```bash
# Our parser's AST (JSON)
cargo run -p tsv_cli parse tests/fixtures/typescript/statements/if/basic/input.svelte --pretty

# Expected AST (source of truth)
cat tests/fixtures/typescript/statements/if/basic/expected.json | jq '.'

# Official parsers for comparison
cargo run -p tsv_debug canonical_parse tests/fixtures/svelte/blocks/each/basic/input.svelte --parser svelte
cargo run -p tsv_debug canonical_parse tests/fixtures/typescript/statements/if/basic/input.svelte --parser typescript
```

### Common Failure Modes

Failure → diagnostic — fix:

- Parser (AST mismatch) → `diff <(jq . expected.json) <(cargo run -p tsv_cli parse input.svelte --pretty | jq .)` — May be stale: `deno task fixtures:update:parsed <pattern>`. Otherwise fix parser.
- Formatter (not idempotent) → `cargo run -p tsv_debug compare input.svelte` — May be stale: `deno task fixtures:update:formatted`. Otherwise fix formatter — never adjust fixtures.
- Validation (structure) → Read the error message — Delete identical files, ensure `expected.json` exists, check naming conventions.

Fixtures exist to catch bugs — they're doing their job when they fail. Never adjust fixtures to work around formatter bugs; fix the code instead.

#### Unformatted Variant Doesn't Normalize (Prettier Variant Discovery)

**Symptom**: Validation error: "unformatted_*.svelte variants don't normalize to input.svelte"

This means prettier doesn't normalize the variant to match the baseline. **This is usually a prettier quirk.**

**Diagnostic procedure:**

```bash
# Step 1: Check what prettier does with the variant
cargo run -p tsv_debug format_prettier tests/fixtures/css/at_rules/container_spacing_prettier_divergence/unformatted_ours_compact.svelte 2>/dev/null > /tmp/prettier_variant.svelte

# Step 2: Compare with input.svelte
diff tests/fixtures/css/at_rules/container_spacing_prettier_divergence/input.svelte /tmp/prettier_variant.svelte
```

**If outputs differ:** Prettier preserves some formatting difference. Continue to Step 3.

```bash
# Step 3: Check if prettier is idempotent with the variant
cargo run -p tsv_debug format_prettier /tmp/prettier_variant.svelte 2>/dev/null > /tmp/prettier_variant2.svelte
diff /tmp/prettier_variant.svelte /tmp/prettier_variant2.svelte
```

**Interpretation:**

- Outputs identical (no diff) — ✅ **Prettier quirk found!** Create `_prettier_divergence/` fixture (see Step 4)
- Outputs differ — ⚠️ Prettier is not idempotent - investigate further (possible prettier bug)

**Step 4: Create prettier_variant fixture**

1. **Create new directory** with `_prettier_divergence` suffix:
   ```bash
   mkdir tests/fixtures/css/at_rules/feature_name_prettier_divergence
   ```

2. **Move files** from base fixture:
   - `input.svelte` → our normalized canonical output (proper spacing)
   - `prettier_variant_X.svelte` → the quirky variant that prettier preserves
   - `README.md` → document the quirk with spec references

3. **Update base fixture**: Add README pointing to the divergence fixture (if relevant)

4. **Generate expectations**: `deno task fixtures:update:parsed feature_name_prettier_divergence`

5. **Validate**: `deno task fixtures:validate feature_name_prettier_divergence`

**Example:** See `container_spacing_prettier_divergence/` - prettier preserves compact spacing in `@container` and `@media` queries but normalizes it in `@supports`.

### Advanced Debugging

#### Compare Formatter Output Directly

```bash
# Quick comparison with prettier
cargo run -p tsv_debug compare --content '<div><div>text</div><div>text</div></div>'

# Compare specific fixture
cargo run -p tsv_debug compare tests/fixtures/svelte/elements/block_siblings/input.svelte
```

#### AST Round-Trip Verification

```bash
# Single file (parse → format → parse → compare ASTs)
cargo run -p tsv_debug ast_diff input.svelte

# Two files (compare ASTs)
cargo run -p tsv_debug ast_diff input.svelte output_prettier.svelte

# Inline content
cargo run -p tsv_debug ast_diff --content '<div>test</div>' --parser svelte
```

#### Measuring Line Widths

When creating `long` fixtures or debugging line wrapping behavior, use `line_width` to measure line lengths:

```bash
# Measure all lines in a file
cargo run -p tsv_debug line_width tests/fixtures/css/values/functions/gradient_long/input.svelte

# Output shows:
# Line 1: 7 chars (0 tabs = 0, content = 7) ✓
# Line 4: 101 chars (3 tabs = 6, content = 95) ✗ EXCEEDS printWidth (100)
# Line 5: 100 chars (3 tabs = 6, content = 94) ⚠️ EXACTLY printWidth (100)
# ...
# Summary: 3/35 lines exceed printWidth (100)
```

**Measure specific line:**

```bash
cargo run -p tsv_debug line_width input.svelte --line 4
# Line 4: 101 chars (3 tabs = 6, content = 95) ✗ EXCEEDS printWidth (100)
#   			clip-path: polygon(...);
```

**Tab width calculation:**

- Each `\t` counts as `tabWidth` chars (default: 2, matching prettier's `tabWidth: 2`)
- **Indentation tabs DO count** toward line length (matches prettier's behavior)
- Total = (tab_count × tabWidth) + content_length

**Boundary testing for `long` fixtures:**

- **99 chars** → must not wrap (✓)
- **100 chars** → boundary behavior (⚠️)
- **101 chars** → must wrap if feature supports wrapping (✗)

**Common use cases:**

- Verify test data actually exceeds 100 chars
- Fix `long` fixtures to have exactly 101 chars for precise boundary testing
- Debug why lines do/don't wrap during implementation
- Validate tab width calculations match prettier

**JSON output for tooling:**

```bash
cargo run -p tsv_debug line_width input.svelte --json
# {"lines": [{"line": 1, "total": 45, "tabs": 1, ...}, ...]}
```

#### Inspect Test Implementation

Key test files:

- `tests/fixtures_tests.rs` - Unified fixture validation (parser + formatter)
- `crates/tsv_debug/src/fixtures/` - Fixture data model (`model.rs`), discovery (`discovery.rs`), and variant discovery (`variants.rs`)
- `crates/tsv_debug/src/fixtures/validation/` - Validation logic: structure rules (`structure.rs`), per-phase checks (`phases.rs`), typed errors (`errors.rs`), summary printing (`summary.rs`)
- `crates/tsv_debug/src/cli/commands/fixtures_*.rs` - Fixture generation commands

**See [fixture_workflow.md](./fixture_workflow.md#quick-reference) for command reference.**

---

## Variants and Divergence Patterns

### Variant Rules

**The primary purpose of `unformatted_*` variants is to expose formatter bugs and edge cases.** They stress-test normalization by using unusual formatting that should collapse to the canonical input.

For step-by-step creation, see [fixture_workflow.md Step 6](./fixture_workflow.md#step-6-validate--variants). For naming conventions, see [fixture_naming.md](./fixture_naming.md#variant-file-naming).

**Key rules:**

- Regular directories: `unformatted_*.*` — normalizes with BOTH formatters
- `_prettier_divergence` directories: `unformatted_ours_*.*` — normalizes with our formatter only (S9 prevents `unformatted_*` here)
- `_spaces` variants should include: extra spaces around operators/parens (`( x )`, `< T >`), newlines mid-expression, and mixed indentation. Random mid-construct newlines should collapse; intentional multi-line newlines (after `{`) should preserve.

**Interpreting variant results:**

| Prettier Result        | Our Formatter Result   | Action                                    |
| ---------------------- | ---------------------- | ----------------------------------------- |
| Normalizes to baseline | Normalizes to baseline | Valid variant                             |
| Normalizes to baseline | Differs from baseline  | Formatter bug — fix it                    |
| Preserves difference   | Normalizes to baseline | Enhancement (tsv better than prettier)    |
| Preserves difference   | Keeps the same form    | Not a normalization test — re-home as `variant_*` (dual-stable) |
| Preserves difference   | Rewrites to a 3rd form | Not a normalization test — re-home as `divergent_variant_*`             |

### Formatter Divergence: output_prettier.svelte

**Rarely needed** — only for permanent prettier bugs or spec violations.

#### Decision Framework

**The spec is the source of truth; prettier-matching is the default tie-breaker, not the goal.** When the CSS/JS/Svelte spec defines canonical behavior, tsv follows the spec — even when prettier's output is itself valid. Prettier-matching is what we adopt when the spec is silent or permissive (which is most of the time). So when `output_prettier.svelte` appears (auto-generated by `fixtures_update_formatted`), adopt it unless prettier conflicts with a spec-defined canonical form, has a documented bug, or moves comments to different syntactic positions (see [conformance_prettier.md Comment Position Philosophy](./conformance_prettier.md#comment-position-philosophy)):

```bash
cp output_prettier.svelte input.svelte && rm output_prettier.svelte
deno task fixtures:update
```

- Spec defines a canonical form prettier doesn't emit — Diverge — follow the spec (document with spec refs)
- Prettier output correct or cosmetic difference — Adopt prettier
- Prettier has documented bug / violates spec — Consider divergence (needs strong justification + README)
- Prettier moves comments to different position — Diverge — preserve user's comment placement
- "I prefer our way" or "not implemented yet" — Adopt prettier

The empty custom-property value divergence ([CSS: Values](./conformance_prettier.md#css-values)) is the canonical example of row 1: every spacing variant is valid CSS that prettier preserves verbatim, but the spec trims the whitespace and defines a single-space serialization, so tsv normalizes to that one form.

**Worked example:**

```
# Found: function_polygon_long/output_prettier.svelte
# Prettier keeps .long case inline (doesn't wrap at threshold)
# Our implementation: wraps at threshold
# Analysis: Prettier's behavior is consistent with other function wrapping
# Decision: Adopt prettier's behavior
cp output_prettier.svelte input.svelte && rm output_prettier.svelte
deno task fixtures:update
```

When `output_prettier.*` exists, prettier baseline validation (F3) is skipped — F2 checks the file matches prettier instead.

---

### Svelte Parser Divergence

For **permanent, intentional** parser differences (spec compliance, Svelte bugs), use `expected_ours.json` + `expected_svelte.json`. Both must exist together; `expected.json` cannot coexist with them. Requires `_svelte_divergence` directory suffix and README.md documenting what differs, why, and spec references.

```
nth_child_of_svelte_prettier_divergence/
├── input.svelte              # li:nth-child(2n of .item) { }
├── expected_ours.json        # Our AST (spec-compliant: selector inside Nth node)
├── expected_svelte.json      # Svelte's AST (flattens selector as sibling)
└── README.md                 # Spec compliance vs Svelte behavior
```

When Svelte's parser is expected to fail: `expected_svelte.json` contains `{ "error": "failed to parse" }`.

Never use for in-progress features or temporary gaps — let the test fail normally. See ./conformance_svelte.md.

---

### Prettier Variant System

> **For complete prettier quirk catalog and decision framework**, see ./conformance_prettier.md.

#### Overview

Document prettier's output when it differs from ours or has multiple stable outputs (quirks).

#### File Patterns

**Pattern 1: Prettier outputs something different**

```
fixture_name_prettier_divergence/
├── input.*                   # Our canonical (formats to itself)
├── output_prettier.*         # What prettier produces
├── unformatted_ours_*.*      # Variants that normalize to input (our formatter only)
└── README.md                 # Explains the difference
```

**Pattern 2: Prettier has multiple stable outputs (quirks)**

```
fixture_name_prettier_divergence/         # Note: _prettier_divergence suffix required!
├── input.*                          # Our canonical (formats to itself)
├── output_prettier.*                # Prettier's normalization target (what prettier(input) produces)
├── prettier_variant_*.*               # Stable outputs prettier preserves (may include normalization target)
├── unformatted_ours_*.*             # Variants that normalize to input (our formatter only)
└── README.md                        # Documents the quirks
```

**Pattern 3: Normalization divergence (prettier(input) == input)**

When prettier formats our input identically but differs in how it normalizes variants:

```
fixture_name_prettier_divergence/
├── input.*                          # Our canonical (prettier(input) == input)
├── unformatted_ours_*.*             # Variants our formatter normalizes to input
└── README.md                        # Documents the normalization difference
```

**Pattern 4: Unstable intermediate output (prettier requires multiple passes)**

When prettier requires multiple passes to reach stable output from a variant:

```
fixture_name_prettier_divergence/
├── input.*                              # Stable output (both formatters agree)
├── unformatted_ours_*.*                 # Source variant (our formatter normalizes in 1 pass)
├── prettier_intermediate_*.*            # Prettier's unstable first-pass output
└── README.md                            # Documents the multi-pass behavior
```

The `prettier_intermediate_*` files capture prettier's intermediate form. For example:

- `unformatted_ours_expanded.svelte` → prettier → `prettier_intermediate_expanded.svelte` (unstable)
- `prettier_intermediate_expanded.svelte` → prettier → `input.svelte` (stable)

This pattern documents that our formatter reaches the stable form in one pass while prettier requires two.

**Pattern 4b: Unstable intermediate converging to a variant (not input)**

Variant of Pattern 4 where prettier's second pass lands on a documented `variant_*`/`prettier_variant_*` instead of `input`:

```
fixture_name_prettier_divergence/
├── input.*                                          # Our canonical (formats to itself)
├── unformatted_ours_*.*                             # Source variant (our formatter normalizes to input)
├── prettier_intermediate_to_variant_*.*             # Prettier's unstable first-pass output
├── variant_*.* (or prettier_variant_*.*)            # The convergence target prettier reaches on the 2nd pass
└── README.md                                        # Documents the D → C → B chain
```

Chain shape:

- `unformatted_ours_X` (D) → prettier → `prettier_intermediate_to_variant_X` (C, unstable)
- `prettier_intermediate_to_variant_X` (C) → prettier → some `variant_*`/`prettier_variant_*` file (B, stable)

Our formatter normalizes D → `input` in one pass; prettier's two-pass walk goes elsewhere.

**Pattern 5: Testing prettier's normalization to its own output**

When you want to test that prettier normalizes certain inputs to `output_prettier.*`:

```
fixture_name_prettier_divergence/
├── input.*                              # Our canonical (formats to itself)
├── output_prettier.*                    # Prettier's canonical output (required)
├── unformatted_prettier_*.*             # Variants that prettier normalizes to output_prettier
└── README.md                            # Documents the divergence
```

- `unformatted_prettier_compact.svelte` → prettier → `output_prettier.svelte`
- These test that prettier normalizes various compact/unformatted inputs to its stable output

**Pattern 6: Dual-stable forms (both formatters keep stable, but different from input)**

When prettier produces a stable output that our formatter also keeps stable (idempotent),
but neither normalizes to `input.*`:

```
fixture_name_prettier_divergence/
├── input.*                              # Our canonical
├── variant_*.*                  # Both formatters keep this stable (NOT normalized to input)
├── unformatted_ours_*.*                 # Optional: source variants
└── README.md                            # Documents the dual-stable forms
```

Both prettier-stable; they differ in our formatter:

- `prettier_variant_*` — normalizes to `input`
- `variant_*` — stable (idempotent), NOT `input`

**Pattern 7: Divergent-variant forms (prettier keeps stable, ours rewrites to a third form)**

When prettier keeps a form stable but our formatter rewrites it to a *third* stable
form — distinct from both the form and `input`, so three stable forms coexist:

```
fixture_name_prettier_divergence/
├── input.*                              # Our canonical
├── divergent_variant_*.*                        # Prettier keeps this; ours rewrites it to a third stable form
├── unformatted_ours_*.*                 # Optional: source variants
└── README.md                            # Documents the divergent-variant forms
```

All three prettier-stable-form kinds differ only in what our formatter does:

- `prettier_variant_*` — ours → `input`
- `variant_*` — ours keeps it (dual-stable)
- `divergent_variant_*` — ours → a distinct third stable form (NOT `input`, NOT the form)

**Notes**:

- In `_prettier_divergence` directories, use `unformatted_ours_*.*` instead of `unformatted_*.*` to indicate that only our formatter normalizes these to input (prettier must NOT normalize to input).
- Use `unformatted_prettier_*.*` when you want to test that prettier normalizes to `output_prettier.*` (requires `output_prettier.*` to exist).
- `output_prettier.*` and `prettier_variant_*.*` **can have identical content** when prettier's normalization target is also one of its quirky preserved forms.

#### Validation Rules

**For `output_prettier.*`:**

- Must equal `prettier(input)` exactly
- Existence implicitly skips prettier baseline test (tsv intentionally differs)

**For `prettier_variant_*.*`:**

1. **Rule 1**: `prettier(variant_file) == variant_file` (prettier is idempotent)
2. **Rule 2**: `tsv_format(variant_file) == input` (tsv normalizes)
3. **Rule 3**: `variant_file != input` (actually different)

**For `unformatted_ours_*.*` (ONLY in `_prettier_divergence` directories):**

- Our formatter validation ALWAYS runs: `tsv_format(unformatted_ours_*) == input`
- Prettier non-normalization check runs: `prettier(unformatted_ours_*) != input` (verifies divergence claim)
- The `_ours` suffix makes it explicit: only our formatter normalizes these to input (see [The `_ours` Naming Convention](#the-_ours-naming-convention))
- **S8 enforces**: `unformatted_ours_*` can ONLY exist in `_prettier_divergence` directories
- If prettier also normalizes to input, the file should be `unformatted_*` instead (validation error)

**For `unformatted_*.*` (in regular directories):**

- Both prettier AND our formatter validate normalization
- Cannot exist in `_prettier_divergence` directories (S9 validation failure)

**For `unformatted_prettier_*.*` (ONLY in `_prettier_divergence` directories with `output_prettier.*`):**

- Requires `output_prettier.*` to exist (validation error otherwise)
- `prettier(unformatted_prettier_*) == output_prettier.*` (prettier normalizes to its canonical output)
- Our formatter validation is NOT applied (these test prettier's behavior, not ours)

**For `prettier_intermediate_*.*` (ONLY in `_prettier_divergence` directories):**

- Must have corresponding `unformatted_ours_*` file (same suffix)
- `prettier(unformatted_ours_X) == prettier_intermediate_X` (captures first-pass output)
- `prettier(prettier_intermediate_X) != prettier_intermediate_X` (must be unstable)
- `prettier(prettier_intermediate_X) == input` (converges to stable form)

**For `prettier_intermediate_to_variant_*.*` (ONLY in `_prettier_divergence` directories):**

- Must have corresponding `unformatted_ours_*` file (same suffix)
- Must have at least one `variant_*` or `prettier_variant_*` file in the fixture (the convergence target)
- `prettier(unformatted_ours_X) == prettier_intermediate_to_variant_X` (captures first-pass output)
- `prettier(prettier_intermediate_to_variant_X) != prettier_intermediate_to_variant_X` (must be unstable)
- `prettier(prettier_intermediate_to_variant_X) ∈ {variant_*, prettier_variant_*}` (converges to a documented variant)
- Use this pattern (instead of `prettier_intermediate_*`) when prettier's two-pass convergence lands on a sibling variant rather than `input`

**For `audit_signature.txt` (auto-generated; only when `output_prettier.*` exists AND prettier is non-idempotent on it):**

- Pins each step of prettier's chain `prettier(output_prettier)`, `prettier^2(output_prettier)`, ..., up to and including the fixed point
- Generated/updated/removed automatically by `deno task fixtures:update:formatted`
- F4 byte-equality-checks the file against the live chain every validation run, catching drift in any intermediate pass that F2 (pass-1 only) would miss
- The audit (`fixtures:audit`) recognizes fixtures with a matching signature and stops flagging them as novel; signature drift surfaces in audit output as a regenerate prompt
- Format: header comments, then `%%PASS=N%%` (or `%%PASS=N (fixed point)%%`) section headers separating exact pass content. Do not edit by hand — regenerate with `fixtures:update:formatted`

**For `variant_*.*` (ONLY in `_prettier_divergence` directories):**

- `prettier(file) == file` (prettier keeps it stable)
- `ours(file) == file` (our formatter keeps it verbatim — true dual-stability)
- `ours(file) != input` (must NOT normalize to input — else use `prettier_variant_*`)
- Content must differ from input and from all `prettier_variant_*` files

**For `divergent_variant_*.*` (ONLY in `_prettier_divergence` directories):**

- `prettier(file) == file` (prettier keeps it stable)
- `ours(file) != input` (must NOT normalize to input — else use `prettier_variant_*`)
- `ours(file) != file` (must NOT be dual-stable — else use `variant_*`)
- `ours(ours(file)) == ours(file)` (the rewritten third form is itself a fixed point)
- Content must differ from input and from all `prettier_variant_*` / `variant_*` files

**Key Design Insight:**

File naming shows validation intent:

- `unformatted_*.*` → Normalizes to input with BOTH prettier and our formatter (only in regular dirs)
- `unformatted_ours_*.*` → Normalizes to input with our formatter, NOT with prettier (only in `_prettier_divergence` dirs)
- `unformatted_prettier_*.*` → Normalizes to `output_prettier.*` with prettier (only in `_prettier_divergence` dirs with `output_prettier.*`)
- `prettier_variant_*.*` → Prettier-stable, our formatter normalizes to input
- `variant_*.*` → Both formatters keep stable, NOT input
- `divergent_variant_*.*` → Prettier-stable, our formatter rewrites to a distinct third stable form (NOT input, NOT the form)
- `prettier_intermediate_*.*` → Documents prettier's unstable intermediate output (requires 2nd pass to stabilize, converges to `input`)
- `prettier_intermediate_to_variant_*.*` → Same as above, but the 2nd pass converges to a `variant_*`/`prettier_variant_*` file instead of `input`

**Example:**

```
scope_complex_prettier_divergence/
├── input.svelte                       # @scope (.card, .panel) { }
├── prettier_variant_spaces.svelte       # @scope ( .card,  .panel ) { } (preserved)
└── unformatted_ours_spaces.svelte     # @scope ( .card ,  .panel ) { } (tsv normalizes)
```

Both test different things:

- `prettier_variant_spaces` proves prettier PRESERVES quirky spacing
- `unformatted_ours_spaces` proves tsv normalizes correctly (no prettier validation)

#### When to Use

⚠️ **RARELY NEEDED** - Use `output_prettier.svelte` only when:

- **Prettier bug** - Prettier violates spec, tsv formats correctly
- **Intentional design choice** - tsv chooses better formatting (very rare)
- **NOT for our in-progress work** - fixtures represent final behavior only

✅ Use `prettier_variant_*.*` when:

- Prettier produces quirky output that it then preserves (has multiple stable outputs)
- File contains **prettier's formatted result** (what prettier outputs), not unformatted input
- Each variant: `prettier(variant) == variant` (idempotent quirk)
- Documenting prettier's quirky behavior
- Can pair with `unformatted_*` where `prettier(unformatted) == prettier_variant` and `tsv(unformatted) == input`

❌ Don't use when:

- Prettier normalizes the variant (use `unformatted_*` instead)
- Testing parser behavior (use `expected_ours.json` pattern)

#### Implicit Skip Behavior

⚠️ **CRITICAL CONCEPT - Implicit Skips:**

When `output_prettier.*` exists, certain validations are **automatically skipped**. This is by design, not a bug.

**What gets skipped:**

- `output_prettier.*` exists → F2 runs (check file matches prettier), F3 skipped; `unformatted_*` files are banned there (S9 — prettier can't normalize them to input)

**What ALWAYS runs:**

- Our formatter validation (F1 - validates our behavior regardless)
- Prettier baseline (F2, F3, F5, or F6 - exactly one of these always runs; F5 replaces F2/F3 when `prettier_nonconvergent.txt` declares prettier has no fixed point, F6 when `prettier_rejects.txt` declares prettier throws on the input)
- Prettier normalization on `unformatted_*` files (N3) — S9 only allows them where input is prettier-stable, including `prettier_variant_*`-style divergence dirs

**Key invariant:** Every `_prettier_divergence` directory must document divergence with one of:

- `output_prettier.*` (what prettier produces when different from input)
- `prettier_variant_*.*` (prettier's stable variants)
- `variant_*.*` (dual-stable forms)
- `divergent_variant_*.*` (divergent-variant forms)
- `unformatted_ours_*.*` + README.md (normalization divergence where prettier(input) == input)
- `prettier_nonconvergent.txt` + README.md (prettier never reaches a fixed point — no oracle exists)
- `prettier_rejects.txt` + README.md (prettier throws on the input — no oracle exists)

#### Example: CSS Comment Whitespace

**Two fixtures testing whitespace preservation quirks:**

```
tests/fixtures/css/tokens/comments/in_property_value_after_colon_prettier_divergence/
├── input.svelte                     # font-size: /* comment */ 12px;  (1 space after :)
├── prettier_variant_spaces.svelte     # font-size:  /* comment */ 12px; (2 spaces)
├── prettier_variant_compact.svelte    # font-size:/* comment */ 12px;   (0 spaces)
└── unformatted_*.svelte             # Normalization variants

tests/fixtures/css/tokens/comments/in_property_value_before_colon_prettier_divergence/
├── input.svelte                     # color /* comment */ : red;  (1 space before :)
├── prettier_variant_spaces.svelte     # color /* comment */  : red; (2 spaces)
├── prettier_variant_compact.svelte    # color /* comment */: red;   (0 spaces)
└── unformatted_compact.svelte       # color /* comment */:red;    (normalizes)
```

Both fixtures test prettier's whitespace preservation quirk around colons.

#### Best Practices

- **Descriptive names**: Variant files should clearly explain WHAT differs (e.g., `prettier_variant_spaces.svelte` for "2 spaces")
- **Document WHY**: Use fixture README to explain WHY the quirk exists
- **Don't overuse**: Only add when documenting real prettier behavior
- **Keep input canonical**: `input.svelte` remains the source of truth for standard formatting

---

### Invalid Syntax Pattern: input_invalid_* Files

Test that parsers correctly **reject** invalid syntax.

#### Overview

`input_invalid_*` files document syntax that is intentionally rejected by parsers. They:

1. Document what's disallowed (reference for users)
2. Test that our parser correctly rejects invalid syntax
3. Prevent regressions (accidentally accepting invalid code)

#### File Naming

- `input_invalid_*.svelte` - Invalid Svelte/TypeScript syntax in Svelte context
- `input_invalid_*.ts` - Invalid TypeScript syntax (rare, file-level only)
- `input_invalid_*.css` - Invalid CSS syntax

Files are placed alongside `input.svelte` in existing fixture directories. Multiple invalid files per fixture are allowed: `input_invalid_decl.svelte`, `input_invalid_param.svelte`, etc.

#### Validation Rules

- `.svelte` — Must fail BOTH our parser AND Svelte's parser
- `.ts` — Must fail BOTH our parser AND acorn-typescript
- `.css` — Must fail BOTH our CSS parser AND Svelte's `parseCss`

#### Example

```
contextual_keywords/async_await_as_identifiers/
├── input.svelte                       # Valid async/await usages
├── expected.json
├── input_invalid_await_const.svelte   # const await = 'a'; (invalid)
├── input_invalid_await_param.svelte   # function fn(await: string) {} (invalid)
└── input_invalid_await_let.svelte     # let await = 'a'; (invalid)
```

#### Best Practices

- **One syntax error per file** - Makes errors clear and specific
- **No `{}` blocks** - Don't wrap code in blocks to make it "valid elsewhere"
- **Minimal content** - Just the invalid syntax, nothing extra
- **No expected.json** - These files don't parse, so no AST output

---

### Combining Fixture Patterns

Combine multiple patterns when testing edge cases with both parser and formatter quirks (e.g., CSS comments where both Svelte and Prettier have quirks).

**Example:** `css/tokens/comments/in_property_value_before_colon_prettier_divergence/` uses all patterns - see [Prettier Variant System](#example-css-comment-whitespace) above.

#### Validation Summary

Pattern → validation applied — notes:

- `expected_ours.json` → Our parser must match — Self-consistency required
- `expected_svelte.json` → Svelte parser (informational) — Documents the difference
- `output_prettier.*` → F2: Must equal prettier(input) — When exists, F3 skipped
- (no output_prettier) → F3: prettier(input) must = input — Always checked, even in `_prettier_divergence` dirs
- `prettier_nonconvergent.txt` → F5: prettier(input) ≠ input AND prettier²(input) ≠ prettier(input) — Replaces F2/F3/F4 + prettier-side N rules (no fixed point exists)
- `prettier_rejects.txt` → F6: prettier(input) errors with the pinned substring — Replaces F2/F3/F4 + prettier-side N rules (prettier throws on the input)
- `prettier_variant_*.*` → Rule 1 (prettier idempotent) — Rule 3 (differs from input)
- `unformatted_*.*` → Both formatters validated — N3 (prettier → input) + N4 (ours → input)
- `input_invalid_*` → Must fail to parse — Both our parser and canonical
- All patterns → Structure validation — Always enforced

---

## Reference

### Fixture Generation Commands

**Three-tier command structure:**

- **`fixtures_update_parsed`** - Updates parser expectations
  - Generates `expected.json` using Svelte parser (default)
  - Generates `expected_ours.json` + `expected_svelte.json` when divergence exists

- **`fixtures_update_formatted`** - Updates formatter outputs
  - Generates `output_prettier.*` when prettier differs from input
  - Auto-deletes `output_prettier.*` if identical to input

- **`fixtures_update`** - Updates everything
  - Calls both `fixtures_update_parsed` and `fixtures_update_formatted`
  - Convenience command for full regeneration

- **`fixtures_validate`** - CI validation
  - Validates all fixtures are up to date
  - Checks structure, file consistency, and idempotency

See implementation in `crates/tsv_debug/src/cli/commands/fixtures_*.rs`

### Related Documentation

- **./conformance_prettier.md** - Prettier quirk catalog and validation system
- **./conformance_svelte.md** - Svelte parser compatibility documentation
- **./fixture_naming.md** - Fixture naming conventions
- **[CLAUDE.md](../CLAUDE.md)** - High-level project and fixture concepts
