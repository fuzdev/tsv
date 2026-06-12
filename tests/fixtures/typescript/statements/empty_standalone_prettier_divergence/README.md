# empty_standalone_prettier_divergence

Both formatters remove standalone `;` statements. Prettier also collapses the blank lines left behind. tsv preserves them.

tsv: removes `;`, preserves blank lines between remaining statements
Prettier: removes `;` and collapses blank lines

## Reason

Blank lines between comments indicate intentional visual grouping. Collapsing them loses structural intent.
