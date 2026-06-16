# optional_paren_non_null_new_callee_prettier_divergence

A non-null assertion sealing a parenthesized optional-chain base, used as a `new`
callee. An optional chain can't be a `new` callee per spec, so the parens are
required to seal it. Prettier strips them off both forms, and **both** of its
outputs are themselves syntax errors. tsv keeps the parens (canonical `!`-outside
form).

| Form             | tsv              | Prettier                       |
| ---------------- | ---------------- | ------------------------------ |
| `new (a?.b)!()`  | `new (a?.b)!()`  | `new a?.b!()` (syntax error!)  |
| `new (a?.())!()` | `new (a?.())!()` | `new a?.()!()` (syntax error!) |

## Reason

Semantic preservation. `new (a?.b)!()` constructs on the asserted result of `a?.b`;
dropping the parens changes (or breaks) the meaning. Prettier strips the parens off
**both** forms, and both results fail to re-parse:

- **Member base** `new (a?.b)!()` → `new a?.b!()`, which **fails to re-parse**
  ("Optional chaining cannot appear in the callee of new expressions"). Prettier's
  own output is invalid — a non-idempotency error, not just a layout difference.
- **Call base** `new (a?.())!()` → `new a?.()!()`, which **also fails to re-parse**
  with the same error. (Under prettier-plugin-svelte 3.5.2 this form stayed valid
  as `new (a?.()!)()` — the `!` merely relocated inside the parens; 4.x strips the
  parens here too, so prettier now mangles both forms identically.)

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
