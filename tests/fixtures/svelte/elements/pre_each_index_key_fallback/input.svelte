<!-- {#each} with an index inside <pre>: whitespace is significant, stays inline -->
<pre>{#each items as item, i}{i}{item}{/each}</pre>

<!-- {#each} with a context-less index (no item binding) -->
<pre>{#each items, i}{i}{/each}</pre>

<!-- {#each} with a key -->
<pre>{#each items as item (item.id)}{item}{/each}</pre>

<!-- {#each} with both index and key -->
<pre>{#each items as item, i (item.id)}{i}{item}{/each}</pre>

<!-- {#each} with an {:else} fallback -->
<pre>{#each items as item}{item}{:else}empty{/each}</pre>
