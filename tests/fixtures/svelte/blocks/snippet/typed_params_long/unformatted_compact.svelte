<script lang="ts">
	type A = string;
</script>

{#snippet fn(a: A | 'bbbbb' | 'ccccc' | 'ddddd' | 'eeeee' | 'fffff' | 'ggggg' | null, b: A = '600')}
	<!-- 100 chars - stays inline -->
	<div>text</div>
{/snippet}

{#snippet fn(a: A | 'bbbbb' | 'ccccc' | 'ddddd' | 'eeeee' | 'fffff' | 'ggggg' | null,b: A = '6000')}
	<!-- 101 chars - each param wraps to its own line -->
	<div>text</div>
{/snippet}
