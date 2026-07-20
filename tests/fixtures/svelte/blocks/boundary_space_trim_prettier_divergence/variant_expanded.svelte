<!-- space-only section boundaries are render-free (compile-time trim), so they glue inline -->
{#if cond}
	text1
{/if}

{#if cond}
	text1
{:else}
	text2
{/if}

{#if cond}
	<div>text1</div>
{:else}
	<div>text2</div>
{/if}

{#each items as item}
	text1
{:else}
	text2
{/each}

{#key expr}
	text1
{/key}

{#snippet fn()}
	text1
{/snippet}
