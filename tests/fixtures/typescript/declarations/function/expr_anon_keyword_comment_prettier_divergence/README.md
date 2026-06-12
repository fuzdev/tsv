# Anonymous function expression keyword comment divergence

Prettier relocates comments between the `function`/`function*` keyword and
opening `(` in anonymous function expressions:

- No params: `function /* c */ ()` → `function () /* c */ {}`
- With params: `function /* c */ (x)` → `function (/* c */ x)`
- Generator: `function* /* c */ ()` → `function* () /* c */ {}`
- Export default: `export default function /* c */ ()` → `export default function () /* c */ {}`

We preserve the comment in the user's original position between keyword and
params. Per comment placement policy, user intent is preserved when prettier
moves comments to different syntactic positions.
