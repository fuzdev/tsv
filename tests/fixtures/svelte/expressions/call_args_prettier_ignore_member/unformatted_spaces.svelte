<script lang="ts">
	const a = fn(
		// prettier-ignore
		{ x:   1 },
		  b
	);
</script>

<!-- a template expression is embedded TS: the frozen slice is a raw range in HOST
	coordinates, so this pins the embedding route as well as the rule -->
{fn(
	// prettier-ignore
	{ x:   1 },
	b
)}

{#if a}
	{@const c = fn(
		// prettier-ignore
		{ x:   1 },
		  b
	)}
	{c}
{/if}

<div
	onclick={() =>
		fn(
			// prettier-ignore
			{ x:   1 },
			b
		)}
>
	text
</div>
