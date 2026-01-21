<script>
	let a, b, c, x;
</script>

<!-- Assignment expressions need parens in block conditions -->
{#if (a = b)}content{/if}

{#if (a += b)}content{/if}

{#if (a ||= b)}content{/if}

<!-- With else -->
{#if (a = b)}
	then
{:else}
	else
{/if}

<!-- Nested -->
{#if (a = b)}
	{#if (c = x)}nested{/if}
{/if}
