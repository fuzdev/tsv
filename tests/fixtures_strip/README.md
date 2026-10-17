# Strip fixtures

Fixtures for `tsv_ts::strip`, the source type strip: TypeScript in, JS out, with
every type-only range blanked to whitespace so that every surviving character keeps its line
and UTF-16 column. This is its own tree, apart from `tests/fixtures/`, because its claim is
an output text, not a parse tree or a formatting fixed point; the root `.formatignore` prunes
it, since blanked output is no format fixed point.

## Layout

- `erase/` — each erasable construct, one per fixture
- `keep/` — JS that stays: decorators, comments, TypeScript-looking JS, and
  type-free input (the no-copy fast path)
- `hazards/` — each place where pure blanking would change the parse, and the one-character
  `;` / `(` / `)` substitution that prevents it, with CR, U+2028 and U+2029 variants where a
  line break decides the hazard
- `refuse/` — TypeScript syntax that type blanking cannot erase, refused rather than blanked
- `positions/` — position identity: non-ASCII text inside and outside blanked ranges, tabs,
  CRLF, CR-only, U+2028/U+2029, a BOM, a hashbang
- `files/` — the `.mts` and `.svelte.ts` input shapes

Every input is module code: the tree has no goal marker, its JS guard and its validator parse
a module. What the strip does with a script — read when the source names that goal, or when
no goal is named and the module parse fails — is pinned by Rust tests instead (the strip
module's unit tests and `tests/strip_goal.rs`).

## Files

Every fixture directory holds one input — `input.ts`, `input.mts`, or `input.svelte.ts` —
and exactly one claim:

- `expected.js` / `expected.mjs` / `expected.svelte.js` — the stripped JS, or
- `refusal.txt` — `UnsupportedKind<TAB>line:column<TAB>line:column`, the refused kind and
  its span (1-based line, 0-based UTF-16 column, ECMAScript line terminators).

Plus a `README.md` saying what the fixture pins, and, when the claim departs from an
oracle, `divergences.txt` naming that oracle (`ts-blank-space`, `node`), or holding
`override ts-blank-space` (below), with the reason in the README.

## Oracles

Both claim files are generated, never hand-written, except under an override (below):

```bash
deno task strip:fixtures:update [pattern...]   # write expected.* / refusal.txt
deno task strip:fixtures:oracle [pattern...]   # re-check the committed files
```

The expected output is [ts-blank-space](https://github.com/bloomberg/ts-blank-space)'s,
run over the input with every CR, U+2028 and U+2029 folded to LF and each restored at its
offset afterwards. On an input holding none of them that is ts-blank-space's output byte for
byte. A refusal is whatever ts-blank-space reports through its `onError` callback. Node's
`module.stripTypeScriptTypes` is the second oracle: every fixture is cross-checked against
it, modulo the whitespace Node writes where a blank holds a space (it keeps tabs, and writes
U+00A0, U+2002 or U+FEFF for a blanked non-ASCII character so the byte length holds), and a
refusal must make Node throw too. Every claimed output must also parse as a JS module under
oxc-parser in JS mode, semantic errors included, and keep the position contract under
[Grading tsv](#grading-tsv), or the run fails and that fixture's claim is not written: the
parse is the check that the output is JS at all. Both tasks run under Node and need
`deno task bench:install`; ts-blank-space is pinned in `benches/js/package.json`, Node is
whatever is installed, and the run prints both versions.

**A U+2028 or U+2029 inside a string literal can't be fixtured.** The fold to LF turns it
into a line break inside the string, which TypeScript does not parse, so the derivation has
nothing to restore. The generator fails loudly on such an input (`TypeScript reports … parse
diagnostic(s)`) rather than writing a claim.

Every disagreement with an oracle must be declared in the fixture's `divergences.txt`, and
every declared one must still fire. The accepted ones:

- **ts-blank-space tests for line breaks with LF alone.** A CR-only, U+2028 or U+2029 break
  at a hazard site goes undetected, and the output reads differently (ASI ends a `return`, or
  a line terminator lands before `=>`). The fold above is what makes tsv's expected output
  handle every ECMAScript line terminator; Node handles them too.
- **ts-blank-space blanks U+2028 / U+2029 inside a blanked range** to a space, moving every
  later line. tsv keeps every line terminator verbatim, as Node does.
- **Node writes a statement-final assertion's `;` one column later**, and none when nothing
  follows the statement. The same program; tsv keeps ts-blank-space's bytes.
- **Node misses the `;` before a computed class member after a blanked `declare` field or a
  blanked bodiless signature**, emitting a different program, and the `;` before a generic
  arrow after a blanked bodiless signature, emitting a syntax error.
- **Node writes a `;` before a blanked overload that nothing before it needs** (after a
  declaration ending in `}`). The same program; tsv keeps ts-blank-space's bytes.
- **Node refuses an overload signature as the lone body of an `if`**, and a member named `in`
  after a bodiless member, which it reads as a variance modifier. tsc accepts both.
- **Node leaves `async` alone on its line** before a moved type-parameter `(`, a syntax error.
- **Node blanks type parameters broken across lines in place**, keeping the parameters' `(`
  where ts-blank-space moves it to the `<`. The same program.
- **Node blanks an assertion whose erasure regroups the expression** (`b + c as T / d`),
  emitting a different program, where ts-blank-space refuses it.
- **Node drops a leading BOM.**
- **Node blanks `import type X = require(...)`** as type-only, where ts-blank-space refuses
  it and tsc's `erasableSyntaxOnly` rejects it.

## Overrides

Where ts-blank-space is wrong, a fixture's `divergences.txt` holds `override ts-blank-space`
in place of a plain `ts-blank-space`, and its claim is hand-reviewed rather than derived.
`strip:fixtures:update` never writes it; both tasks validate it instead, as they validate a
derived claim — an expected output must parse under oxc-parser in JS mode and keep the
position contract, a `refusal.txt` must name an `UnsupportedKind` and a span inside the
input — then grade it against both oracles, so ts-blank-space's derivation must still
disagree with it (a stale override fails, like a stale divergence) and a departure from
Node must be declared with a plain `node` line. Node never derives a claim, so it takes no
override. The overrides:

- **ts-blank-space writes no `;` before a generator method or a member named `in` /
  `instanceof`** whose blanked modifier follows a field with no `;` (`a = 1` then `*gen() {}`
  reads as `a = 1 * gen() {}`, a syntax error; `a = x` then `in` then `y` reads `a = x in y`,
  silently a different program). The `;` goes at the modifier, ts-blank-space's own placement
  for a computed key; Node agrees.
- **ts-blank-space leaves an `override` parameter property verbatim**, which is no
  JS and drops the assignment. The claim is the `ParameterProperty` refusal; Node
  refuses it too.
- **ts-blank-space blanks a bodiless signature — an overload, an abstract method, an index
  signature — with no `;` ever**, so an unterminated statement or field before it runs on into a
  next line that continues it — one opening with `[`, `(`, a template, a generic arrow (its
  `<T>` blanks to leave the `(` first), a binary operator, or a member named `in` /
  `instanceof` (`a = 1` then `[k] = 2` reads `a = 1[k] = 2`). The `;` goes at the signature, as
  a blanked `declare` field takes one, and only there. Node agrees on a statement, except
  before a generic arrow, and not in a class body. As the lone body of an `if`, a loop, a label or
  a script's `with`, the signature always becomes `;` (blanked plain, `if (a)` takes the next
  statement as its body), as every statement blanked whole there does. The `with` case is a
  script's alone, so the Rust tests pin it rather than a fixture.
- **ts-blank-space leaves an optional method's `?` and type parameters in place** and inserts
  blanks before them (`m?<T>()` to `m?   <T>()`), which is no JS and moves every later column.
  Node blanks both.
- **ts-blank-space reads a `>>` after an assertion as a lone `>`** (TypeScript's scanner splits
  `>>` until asked), so it blanks `b < c as T >> d` to `b < c >> d`, which regroups. The claim is
  an `AssertionGrouping` refusal; Node blanks it too.
- **Both oracles blank an assertion over a unary operand of `**`** (`-x as T ** 2` to
  `-x ** 2`, likewise `typeof` and `await`), a syntax error. The claim is an
  `AssertionGrouping` refusal.

## Grading tsv

```bash
deno task strip:fixtures:validate [pattern...]   # also: -v, --update
cargo test --test strip_fixtures_tests           # the same gate, in `cargo test`
```

Per fixture, the strip must produce the claim; its output must have the input's UTF-16
length, keep every line terminator at its UTF-16 offset, and hold at every other offset the
input's character, a space, or a `;` / `(` / `)` substitution; and stripping the output again
must return it unchanged and uncopied. A type-free input must come back uncopied.

The position check has a known limit: a space or a `;` / `(` / `)` is allowed at any offset,
not only inside a type-only range or at a hazard site, so deleted JS or a misplaced
`;` is caught only by the output comparison.

Failures the strip does not yet fix are pinned in the ratchet snapshot
`crates/tsv_debug/src/cli/commands/strip_fixtures_known.txt`: a new failure fails the gate,
and so does a pinned one that stopped failing — re-pin with
`deno task strip:fixtures:validate:update` once the change is deliberate. A broken fixture
(an expected output that breaks the contract above or does not parse under `tsv_ts::parse`,
or a malformed `divergences.txt`) and a panic are never pinned. A reviewed claim is graded
exactly like a generated one. That parse is a TypeScript one, so it shows the expected output
parses, not that it is JS; the oracle side's oxc JS-mode parse is the JS-validity
guard.
