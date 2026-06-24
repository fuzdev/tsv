# namespace_svelte_divergence

Svelte's parser doesn't support CSS attribute namespace selectors (`[ns|attr]`). tsv implements the full CSS Selectors Level 4 syntax.

## Four namespace forms

| Syntax       | Meaning                       |
| ------------ | ----------------------------- |
| `[svg\|href]` | Attribute in named namespace |
| `[*\|title]`  | Attribute in any namespace   |
| `[\|lang]`    | Explicit no namespace        |
| `[class]`     | Implicit no namespace        |

All six attribute matchers (`=`, `^=`, `$=`, `*=`, `~=`, `\|=`) work with namespace prefixes. The dash-match `\|=` can combine with namespaces: `[xml\|lang\|="en"]` (two vertical bars).

**Disambiguation**: After identifier + `|`, if next char is `=` → dash-match operator; otherwise → namespace prefix.

## Fixture Structure

- `expected_ours.json` — tsv's spec-compliant output (source of truth)
- No `expected.json` because Svelte's parser fails on this syntax

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).
