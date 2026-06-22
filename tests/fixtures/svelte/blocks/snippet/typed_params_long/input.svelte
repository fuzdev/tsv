<script lang="ts">
	type A = string;
</script>

<!-- 100 chars - stays inline -->
{#snippet fn(a: A | 'bbbbb' | 'ccccc' | 'ddddd' | 'eeeee' | 'fffff' | 'ggggg' | null, b: A = '600')}
	<div>text</div>
{/snippet}

<!-- 101 chars - each param wraps to its own line -->
{#snippet fn(
	a: A | 'bbbbb' | 'ccccc' | 'ddddd' | 'eeeee' | 'fffff' | 'ggggg' | null,
	b: A = '6000',
)}
	<div>text</div>
{/snippet}
