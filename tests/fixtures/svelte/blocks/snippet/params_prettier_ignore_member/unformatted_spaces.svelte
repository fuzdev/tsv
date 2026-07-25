<script lang="ts">
	let n   = 1;
</script>

<!-- snippet parameters route through the same parameter printer -->
{#snippet a(
	// prettier-ignore
	p:   {x:   1},
		q:   U
)}
	<div>text</div>
{/snippet}

{#snippet b(
	p:   T,
	// prettier-ignore
	q:   {x:   1}
)}
	<div>text</div>
{/snippet}
