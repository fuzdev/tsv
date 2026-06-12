# last_block_prettier_divergence

Prettier has a position-dependent quirk: it expands if-blocks with symmetric spaces (`{#if a} content {/if}`) to multiline, except for the **last block in a file** which stays inline.

tsv: expands consistently regardless of position
Prettier: expands all blocks except the last one in the file

A single block alone appears to stay inline, but only because it's last — not by design.

## Reason

tsv expands blocks consistently. Position in the file should not affect formatting output.
