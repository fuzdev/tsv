<!-- An empty branch collapses: no body, markers glue — never a blank line. -->

<!-- {#if}: empty :else -->
{#if cond}
	<p>a</p>
{:else}{/if}

<!-- {#if}: empty consequent -->
{#if cond}{:else}
	<p>a</p>
{/if}

<!-- {:else if}: empty branch -->
{#if cond}
	<p>a</p>
{:else if other}{:else}
	<p>b</p>
{/if}

<!-- {#each}: empty :else fallback -->
{#each items as item}
	<p>a</p>
{:else}{/each}

<!-- {#each}: empty body -->
{#each items as item}{:else}
	<p>a</p>
{/each}
