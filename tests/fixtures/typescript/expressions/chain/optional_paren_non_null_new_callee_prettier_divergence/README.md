# optional_paren_non_null_new_callee_prettier_divergence

A non-null assertion sealing a parenthesized optional-chain base, used as a `new`
callee. An optional chain can't be a `new` callee per spec, so the parens are
required to seal it. Prettier mishandles them — *inconsistently* — and one of its
outputs is itself a syntax error. tsv keeps the parens (canonical `!`-outside form).

| Form             | tsv              | Prettier                        |
| ---------------- | ---------------- | ------------------------------- |
| `new (a?.b)!()`  | `new (a?.b)!()`  | `new a?.b!()` (syntax error!)   |
| `new (a?.())!()` | `new (a?.())!()` | `new (a?.()!)()` (`!` relocated) |

## Reason

Semantic preservation. `new (a?.b)!()` constructs on the asserted result of `a?.b`;
dropping the parens changes (or breaks) the meaning. Prettier is self-inconsistent:

- **Member base** `new (a?.b)!()` → `new a?.b!()`, which **fails to re-parse**
  ("Optional chaining cannot appear in the callee of new expressions"). Prettier's
  own output is invalid — a non-idempotency error, not just a layout difference.
- **Call base** `new (a?.())!()` → `new (a?.()!)()`: Prettier keeps the parens but
  relocates the `!` inside them. Valid, but a different paren placement.

tsv keeps the parens in the canonical `!`-outside form for both. The `!` is
type-only, so its position relative to the parens carries no runtime meaning, and
tsv normalizes to the outside form — matching the boundary sibling `(a?.b)!.c`.

The bare forms (`new a?.b()`, `new a?.b!()`) are syntax errors in both tsv and
acorn — see `input_invalid_*` here and in the related fixtures.

## Related

- `chain/optional_paren_new_tagged_boundary/` — the non-`!` new-callee + tag cases, where tsv and Prettier agree.
- `chain/optional_paren_non_null_tag_boundary/` — `` (a?.b)!`tpl` ``, where tsv and Prettier agree (Prettier keeps the parens here).
- `chain/optional_paren_non_null_boundary/` — `(a?.b)!.c`, `(a?.b)!()`, `(a?.b)![c]`, the non-null boundary forms.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§TypeScript (Optional-chain non-null new callee).
