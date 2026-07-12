<!-- {@render} / {@debug} tags inside <pre>: whitespace is significant, so they format inline without injecting rendered whitespace -->
<pre>{@render fn()}</pre>

<pre>{@debug a, b}</pre>

<!-- bare {const} / {let} declaration tags directly inside <pre> -->
<pre>{const x = 1}{let y = 2}</pre>

<!-- {@const} inside a block nested in <pre> (its valid placement) -->
<pre>{#each items as item}{@const doubled = item}{doubled}{/each}</pre>
