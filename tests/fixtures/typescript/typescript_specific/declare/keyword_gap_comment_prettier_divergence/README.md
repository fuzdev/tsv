# Divergence: `declare`→head keyword-interior comments (preserve), every head

A block comment inside a declaration header keyword — between `declare` and the head it
modifies, and between the head's own words — stays where the author wrote it. Prettier
**relocates** every one of them past the keyword's last word, onto the name.

```ts
// tsv (preserve)                          // prettier (relocate onto the name)
declare /* a */ namespace /* b */ N {}     declare namespace /* a */ /* b */ N {}
```

This is the whole-family statement of the rule
[conformance_prettier_ts_comments.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier_ts_comments.md#comments-inside-a-multi-word-keyword)
already argues, and its third argument is the one this fixture makes concrete:
**relocation collapses a distinction.** With comments on both sides of the head keyword
prettier lands them in one place (`/* a */ /* b */`), so "annotates the ambient-ness" and
"annotates the name" become indistinguishable. Text survives; the association does not.

Every head `declare` takes is here — `namespace`, `module` (both the identifier and the
string-literal name), `interface`, `enum`, `type`, `class`, `abstract class`, `function`,
`const` — plus the `export` prefix. The heads are not separate rules: each is located
word by word by `Printer::build_keyword_words_doc`, so a gap between any two words is
scanned rather than measured over.

`const enum` is the one **three**-word head, so it carries three positions
(`declare /* d2 */ const /* d3 */ enum P`); the `const`→`enum` gap is an interior gap
like any other, not a separator.

`global` is the exception that proves the shape: it is keyword *and* name at once, so it
has no keyword→name gap for a comment to be relocated **into**, and prettier keeps that
one where the author wrote it — the only row here where the two formatters agree.

The variable heads are cataloged separately with their own `let`/`var`/multi-declarator
coverage — [declarations/variable/declare_keyword_comment](../../../declarations/variable/declare_keyword_comment_prettier_divergence/);
`const h: number` appears here as the cross-family control.

In `declare`'s own gap only the **block** form exists: `declare` carries a
`[no LineTerminator here]`, so a line comment — or a block comment containing a newline —
ASI-splits the statement in two in both formatters, leaving no gap to preserve. See
[syntax/contextual_keywords/declaration_keyword_own_line](../../../syntax/contextual_keywords/declaration_keyword_own_line/).

`const`→`enum` carries no such restriction, so it is the one gap here a **line** comment
reaches (`const // d5⏎↹enum R {}`, and behind `declare`). It takes the
[uniform forced-continuation indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
that `export // c⏎↹default 1;` takes at the same position; prettier relocates it past `enum`
and pulls the name back flush, the line-comment shape of the same relocation.

See also [§Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
