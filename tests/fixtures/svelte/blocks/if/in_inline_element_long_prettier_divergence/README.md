# in_inline_element_long_prettier_divergence

Prettier tolerates exceeding printWidth (103 chars) for short comparison expressions in block conditions like `{#if typeof x === 'string'}` inside inline elements.

tsv: breaks the expression to stay under 100 chars
Prettier: keeps on one line at 103 chars

## Reason

tsv respects the configured printWidth. Prettier's tolerance for slight overruns with short binary expressions is a quirk, not a spec requirement.
