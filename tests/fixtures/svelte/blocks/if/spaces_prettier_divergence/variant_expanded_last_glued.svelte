<!-- space-only section boundaries are render-free (compile-time trim), so they glue inline -->
{#if a}
	<div>text</div>
{/if}

{#if a}
	t
{/if}

{#if a}
	<div>text1</div>
{:else if b}
	<div>text2</div>
{/if}

{#if a}
	t1
{:else if b}
	t2
{/if}

{#if a}

{/if}

{#if a}
	<!-- c --> t3
{/if}

{#if a}<div>text</div>{:else}{/if}
