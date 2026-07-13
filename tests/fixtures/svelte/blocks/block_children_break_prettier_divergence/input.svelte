<!-- Consecutive block elements in if block should break -->
{#if cond}
	<div>text1</div>
	<div>text2</div>
{/if}

<!-- Consecutive block elements in each block -->
{#each items as item}
	<p>text1</p>
	<p>text2</p>
{/each}

<!-- Consecutive block elements in key block -->
{#key expr}
	<p>text1</p>
	<p>text2</p>
{/key}

<!-- Single block element in if block stays inline -->
{#if cond}<div>text</div>{/if}

<!-- Block + else with blocks -->
{#if cond}
	<div>text1</div>
	<div>text2</div>
{:else}
	<div>text1</div>
	<div>text2</div>
{/if}
