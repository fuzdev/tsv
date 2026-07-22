<!--
	A multiline block body lays out block-style: head and tail intact, body on its own
	indented lines — regardless of how the author wrote the render-free boundary.
-->
{#if cond}<div>a</div>
	<div>b</div>
{/if}

{#each items as item}<div>{item}</div>
	<div>b</div>
{/each}

{#key expr}<div>a</div>
	<div>b</div>
{/key}

{#snippet name(param)}<div>{param}</div>
	<div>b</div>
{/snippet}

{#await promise}<div>a</div>
	<div>b</div>
{:then value}<div>{value}</div>
	<div>b</div>
{:catch error}<div>{error}</div>
	<div>b</div>
{/await}

<!-- Every branch boundary, not just the first -->
{#if cond}<div>a</div>
	<div>b</div>
{:else if other}<div>c</div>
	<div>d</div>
{:else}<div>e</div>
	<div>f</div>
{/if}

<!-- A body that fits on one line still collapses inline -->
{#if cond}<div>a</div>{/if}
