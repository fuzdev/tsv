# Anonymous function expression line comment divergence

Prettier relocates line comments between the `function`/`function*`/`async function`
keyword and opening `(` in anonymous function expressions. Not idempotent — takes
2 passes to stabilize:

Pass 1 (from our input):
- No params: `function // c\n()` → `function () // c\n{}`
- With params: `function // c\n(x)` → `function (\n\t// c\n\tx\n)`
- Generator/async/export default: same as no-params

Pass 2 (stable form):
- No params/generator/async/export default: `function () // c\n{}` → `function () {\n\t// c\n}`
- With params: unchanged (already stable from pass 1)

We preserve the comment in the user's original position between keyword and
params. Per comment placement policy, user intent is preserved when prettier
moves comments to different syntactic positions.

`output_prettier.svelte` is prettier's first-pass output. `variant_in_body.svelte`
is the stable form (both formatters keep idempotent).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
