# empty_standalone_prettier_divergence

Both formatters remove standalone empty `;` statements. The divergence is the
blank lines left behind: tsv preserves them, prettier collapses them.

tsv: removes `;`, preserves blank lines between remaining statements
Prettier: removes `;` and collapses blank lines

Both the spaced (`input`) and collapsed (`variant_standalone`) forms are
dual-stable in both formatters — the divergence shows only when the `;`-bearing
source (`unformatted_ours_standalone`) is normalized: tsv keeps the blank lines,
prettier removes them to reach `variant_standalone`.

## Reason

Design choice. Blank lines between comments indicate intentional visual
grouping; collapsing them loses structural intent.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §TypeScript.
