# Spec-conformant character reference decoding

Four character references that HTML5 decodes and Svelte's decoder
(`1-parse/utils/html.js`) does not. Each is a slip in the implementation rather
than a choice Svelte made, so tsv decodes them. See
[conformance_svelte.md §Entity Decoding Corrections](../../../../../../docs/conformance_svelte.md#entity-decoding-corrections)
for the catalog entry.

## The four cases

**Uppercase hex** (`&#X41;` → `A`). The
[numeric character reference state](https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-state)
opens a hex reference on either `U+0078 x` or `U+0058 X` — the two are adjacent
entries in one switch. Svelte's pattern, `#(?:x[a-fA-F\d]+|\d+)(?:;)?`, spells only
the lowercase one.

**A zero code** (`&#0;` → NUL). Svelte's `if (!code) return match` guards the decode
against an unknown or unparseable reference; a code of `0` is merely the other falsy
value, caught in passing. The spec's
[numeric character reference end state](https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-end-state)
maps `0x00` to U+FFFD, but tsv answers NUL — the sentinel Svelte's `validate_code`
deliberately uses for every code it will not emit (a surrogate half, a code past the
last supported plane). Correcting the guard without adopting a second sentinel is what
keeps `&#0;` and `&#xD800;` the same answer; the latter agrees with Svelte and is
pinned by [numeric_out_of_range](../numeric_out_of_range/).

**An omitted plane** (`&#x30000;` → U+30000). The
[numeric character reference end state](https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-end-state)
replaces only a surrogate half and a value past U+10FFFF; every other code point is
emitted as itself. Svelte's `validate_code` instead enumerates the planes it will
emit — planes 0–2 and two ranges of plane 14 — and drops everything else to NUL, so
an assigned character (U+30000 is CJK Extension G) or a private-use one is destroyed.
That the enumeration is a slip rather than a policy is visible in its history:
[sveltejs/svelte#15823](https://github.com/sveltejs/svelte/pull/15823) is a *fix*
adding the plane-14 ranges after a user hit the hole. The contrast cases here pin the
two planes Svelte does admit, where the decoders agree.

**The attribute-value boundary** (`&AMP_` → `&_` in an attribute). The
[named character reference state](https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state)
holds a semicolon-less reference literal only before `=` or an **ASCII alphanumeric**.
Svelte reaches for JS's `\b`, whose word class also holds `_` — while its own comment
beside the regex quotes the spec rule ("next character is =, number or alphabet"), so
the intent and the code disagree. The rest of that rule is unaffected: `\b` is ASCII, so
Svelte and tsv agree that a non-ASCII letter or digit does *not* hold the reference —
pinned by
[attributes/entity_no_semicolon_boundary](../../../attributes/entity_no_semicolon_boundary/).

## Contrast cases

The second half of the fixture is the same four questions asked where both decoders
agree: a lowercase hex marker, a nonzero code, a plane Svelte admits, and a
semicolon-less reference before `-` (decodes) and before `=` (does not).

Formatting is unaffected throughout — the printer emits the source text of a
reference verbatim, so this is a parse-side divergence only.
