# Parser correction: a reserved word in a heritage clause — Svelte Divergence

A heritage element is a type **reference** — `TypeReference: TypeName`, and
`TypeName: IdentifierReference | NamespaceName . IdentifierReference`. A reserved
word is never an `IdentifierReference`, so `extends void` / `null` / `true` / `this`
/ `super` are grammar errors. acorn-typescript accepts all five (its heritage reader
takes any `IdentifierName`); tsv rejects them, uniformly.

The three oracles disagree here in three different directions, so the rule is chosen
rather than inherited:

| heritage element | tsc parser | acorn | prettier | tsv |
| --- | --- | --- | --- | --- |
| `A` `number` `string` `any` `undefined` `A.B` `A<T>` | accept | accept | accept | **accept** |
| `void` | **TS1109** | accept | reject | **reject** |
| `null` `true` `this` | accept | accept | **reject** | **reject** |
| `super` | **TS1034** | **accept** | reject | **reject** |
| `1` `'s'` `(A)` `typeof A` `[A]` `A[]` `{a: 1}` | mostly accept | reject | reject | **reject** |

tsv follows the **grammar**, which is exactly prettier's line — its error states the
rule outright: *"Interface declaration can only extend an identifier/qualified name
with optional type arguments."* tsv matches prettier on every row above.

`let`, `yield` and `await` are the edges that *look* alike and are not, so they get
their own table — and they split on the actual rule, not on the resemblance:

| heritage element | acorn | prettier | tsv |
| --- | --- | --- | --- |
| `let` `yield` | accept | accept | **accept** |
| `await` | accept | accept | **reject** (Module goal) |

Neither `let` nor `yield` is barred here by the reserved-word rule. `let` is not a
`ReservedWord` at all; `yield` is one, but `IdentifierReference[Yield]` readmits it
under `[~Yield]`. What is left in both cases is the strict-mode early error of
ecma262 §sec-identifiers-static-semantics-early-errors, which tsv defers — so
accepting them is the same rule that already reads them as ordinary type names
everywhere else (`let x: let`, `type T = yield.Foo`, `typeof let`). Pinned by the
siblings [heritage_let](../heritage_let/) and [heritage_yield](../heritage_yield/).

The `[~Yield]` / `[~Await]` guards do come along, though — this head really is an
`IdentifierReference`, so inside a generator or async function the words are the
operator and the name reading is unreachable: `function* g() { interface A extends
yield {} }` rejects, as it does for tsc (TS1109, reached by parsing heritage with
its *expression* parser). A plain type annotation is unaffected in both.

`await` carries one more bar on top, and not the reserved-word one: the **goal**
axis (`BindingIdentifier`/`IdentifierReference : await` is a Syntax Error when the
goal is `Module`), which tsv enforces as a settled decision rather than defers. At
`Goal::Script`, outside a `[+Await]` context, it is accepted like the other two.
Prettier accepts it under both goals — the one place its heritage line is looser
than the rule it states.

No tsc column on that table: the compiler's own corpus carries no
`interface … extends let` / `yield` case to read a baseline from, and its two
`extends await` hits are value-position *class* heritage, a different production.
Probed directly, tsc's parser accepts `interface A extends let {}` and
`… extends yield {}`.

The other two are lenient for structural reasons, not by decision. **acorn** reads
the heritage name as a bare `IdentifierName`, so every reserved word slips through.
**tsc** parses heritage with its *expression* parser (`parseLeftHandSideExpressionOrHigher`)
and defers primitive-ness to the checker, which is why literals and parenthesized
expressions get in — and why `void` and `super`, which are not left-hand-side
expressions either, are the two it still rejects. "tsc's parser accepts" means only
*not a grammar error*; it is a weak signal on its own (see the oracle note in
[conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript
Corrections).

The **contextual** type keywords go the other way and are accepted, since they are
ordinary identifiers (`let string = 1` is legal): `interface A extends number {}` is
pinned by the sibling [heritage_type_keyword](../heritage_type_keyword/), and `let`
by [heritage_let](../heritage_let/). This fixture is the boundary between the two.

Because the canonical parser accepts these inputs, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject). This
`tsv_rejects.txt` fixture pins the divergence from the other side: tsv rejects
(`tsv_rejects.txt` substring), while `expected_svelte.json` proves acorn still
accepts.

The same reserved-vs-contextual line is drawn in the qualified-name **head**
position by
[reserved_keyword_qualified_head](../../reserved_keyword_qualified_head_svelte_divergence/);
the **tail** position after a `.` takes a full `IdentifierName` and admits reserved
words, pinned by
[reserved_keyword_qualified_tail](../../reserved_keyword_qualified_tail/).

**Upstream**: @sveltejs/acorn-typescript — `tsParseHeritageClause` accepts any
`IdentifierName`, including the reserved words.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§TypeScript Corrections (Reserved word in a heritage clause).
