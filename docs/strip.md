# The Type Strip

`tsv strip` turns TypeScript into JS by **blanking**: every type-only range becomes
whitespace, and nothing else is rewritten. No transform, no configuration, no sourcemap —
the output needs none, because every surviving character is where it was. The verdicts are
TypeScript's erasure semantics, the ones [ts-blank-space](https://github.com/bloomberg/ts-blank-space)
and Node's `module.stripTypeScriptTypes` share. The implementation is `tsv_ts::strip`
(`crates/tsv_ts/src/strip/`); the fixtures that define its output are
[tests/fixtures_strip/](../tests/fixtures_strip/README.md).

## Surfaces

- **Rust**: `tsv_ts::strip(source, &arena) -> Result<Stripped, StripError>`, the unset goal,
  and `tsv_ts::strip_with_goal(source, goal: Option<Goal>, &arena)`. `Stripped.code`
  borrows the source when nothing is type-only. `StripError::Parse` carries the
  `ParseError`; `StripError::Unsupported { kind, span }` the refused construct
  (`UnsupportedKind::name` is its snake_case wire name) and its byte span.
  `StripError::message(source)` renders either as the CLI and the packages print it.
- **npm**: `strip_typescript(source, options?)` in `@fuzdev/tsv-wasm` and the native
  `@fuzdev/tsv` — the `strip` cargo feature of `tsv_wasm` and `tsv_napi`, in each crate's
  default set, which the format-only and parse-only wasm packages build without. It
  returns a result instead of throwing for what it finds in the source: `{ok: true, code}`
  or `{ok: false, error}` with `error.kind` `'parse'` (the `message`, `start` and `loc` the
  `SyntaxError` from `parse_typescript` carries for the goal the error belongs to) or `'unsupported'` (`construct`, and the
  span as `start` / `end` / `loc: {start, end}`). The bag takes `sourceType` as a format's
  does. Argument misuse throws a `TypeError`, as on
  every export ([crates/tsv_wasm/CLAUDE.md §The npm Facade](../crates/tsv_wasm/CLAUDE.md#the-npm-facade-options--typed-returns)).
- **CLI**: `tsv strip <file> | --content <s> | --stdin [--source-type <t>] [--check]`, in the
  native binary and
  the JS mirror alike ([cli.md §Stripping types](./cli.md#stripping-types-tsv-strip)).

## The parse goal

The strip reads the goal as `format` does, one rule for the two: a named goal (`sourceType`,
`--source-type` with `--content` / `--stdin`, `Some(goal)`) is exact, and with none named the
source is parsed as a **module, retried as a script** only if that fails — so a legacy
sloppy script (a `with` statement, a leading-zero literal, `await` as a name) strips, while
a module-valid source is never reinterpreted. When both attempts fail, the error reported
is the one `format` reports ([cli.md §Multi-File Formatting](./cli.md#multi-file-formatting)).
A CLI path whose extension settles the goal — `.mts`, a module by name — takes no retry;
`--source-type` beside a path is refused, as on `format`. With no goal named the result does
not say which grammar read the source, so a loader that runs the output names the goal it
will evaluate under (`sourceType`) rather than relying on the retry.

The output is JS at the goal the source was read at: a sloppy script strips to a sloppy
script. No verdict depends on the goal, except that a script's `with` body is one more lone
statement body: a statement blanked whole there becomes `;`, as under an `if`, where
ts-blank-space blanks an overload plain and hands the next statement to the `with`.

## The position invariant

Every surviving character keeps its line and its UTF-16 column, which is what an engine
reports in a stack trace:

- a blanked character becomes one space per UTF-16 unit (two for an astral character), so
  UTF-16 offsets never move and the output is never longer than the input — byte offsets
  shift only after non-ASCII text inside a blanked range;
- every ECMAScript line terminator inside a blanked range is kept verbatim — LF, CR (so
  CRLF), U+2028, U+2029 — as Node keeps them, where ts-blank-space blanks U+2028 / U+2029
  to a space;
- a leading byte-order mark and a hashbang are kept, and a type-free source comes back as
  the same string.

The only other bytes the strip writes are single `;`, `(` or `)` substitutions inside a
blanked range, where pure blanking would let the next line continue the expression: a `;`
starting a statement or member blanked whole after JS that does not end in one
(`let x = y⏎type T = 1⏎(z)`), in place of a statement-final `as` / `satisfies`
(`let x = y as T⏎(z)`), or before a blanked bodiless signature after an expression left open;
a `)` moved to the end of an arrow's blanked return type when a line break precedes the
`=>`; and a `(` moved to the start of blanked type parameters when a line break precedes
the parameters' `(`. A line break here is any ECMAScript line terminator. The module doc of
`crates/tsv_ts/src/strip/mod.rs` lists each case.

## Verdicts

**Blanked**: type annotations, return types, type parameters and arguments, `implements`
clauses; `as T` / `satisfies T`, non-null and definite-assignment `!`, optional `?`;
`type` and `interface` declarations, anything `declare`d, a namespace with no value member,
overload signatures, `import type` / `export type`, an inline `type` specifier with its
comma; `abstract` and `declare` class members and index signatures; the
`public` / `private` / `protected` / `readonly` / `abstract` / `override` / `declare`
modifiers on kept members; a leading `this` parameter. A declaration file (`.d.ts`) is no
special case: its `declare`d and type-only items blank, its value `import`s, `export … from`,
`export { … }` lists and `export default x` are kept, and `export =` / `import =` refuse — so
its output is not a module to run (`export as namespace N;` too is kept verbatim, as both
oracles keep it).

**Kept** (JS, with only the type syntax inside blanked): decorators, `accessor` fields, an
import whose braces only held `type` specifiers (`import {} from 'x'` still loads `'x'`, as
`verbatimModuleSyntax` specifies), an unmarked import used only as a type (eliding it would
need symbol resolution), and every comment outside a blanked range.

**Refused** — TypeScript that type blanking cannot erase: a construct with runtime semantics,
or an assertion whose blanking would regroup its expression. The strip refuses rather than
transforms, so the output is always the input with ranges blanked:

| `construct` | what |
|---|---|
| `enum` | a non-`declare` `enum`, `const enum` included |
| `value_namespace` | a non-`declare` `namespace` holding a value member |
| `parameter_property` | `constructor(public x)` and the other accessibility, `readonly` and `override` modifiers on a constructor parameter |
| `import_equals` | `import x = require('y')` / `import x = A.B` |
| `export_equals` | `export = x` |
| `type_assertion` | the angle-bracket assertion `<T>x` |
| `assertion_grouping` | an `as` / `satisfies` whose blanking would regroup the expression around it (`a + b as T / c`, `-a as T ** 2`) |

The first six are what TypeScript's `erasableSyntaxOnly` rejects, so code that type-checks
under it meets only the last, which a pair of parentheses keeps (`a + (b as T) / c`). A
refusal's span is the construct's (for a parameter property, its first modifier).

**Assumed tsc-checked.** tsv's parser is permissive and defers static-semantic early errors
([conformance_tsc.md](./conformance_tsc.md)), so it accepts some value-bearing shapes tsc
rejects — a class field `declare x = 1` or `abstract x = 1`, `declare let a = 1`, a bodied
member of a `declare class`. Each is blanked whole, as ts-blank-space blanks it, so the
value is silently gone; refusing those is the diagnostics layer's job, not the strip's.
tsc also accepts a class declaration as the lone body of an `if`, a loop or a script's
`with` (`if (a) abstract class A {}`), which no JS grammar does; it passes through with only
`abstract` blanked, as both oracles leave it.

## Oracles

ts-blank-space is the expected-output oracle for every fixture and Node the second,
cross-checked (`deno task strip:fixtures:oracle`); the accepted disagreements with each —
line terminators tested beyond LF, U+2028 / U+2029 kept, Node's own substitution spellings —
are listed in [tests/fixtures_strip/README.md](../tests/fixtures_strip/README.md). The real-code
differential is `deno task strip:corpus` ([audits.md §Strip corpus](./audits.md#strip-corpus-differential-stripcorpus)),
and the two engines are held to each other by `deno task engines:audit`.

## Anti-features

- **No transforms** — no enum lowering, namespace flattening or parameter-property
  synthesis: a transform would move positions and need a sourcemap, a different product.
- **No configuration.**
- **No sourcemaps** — the invariant is what makes them unnecessary.
