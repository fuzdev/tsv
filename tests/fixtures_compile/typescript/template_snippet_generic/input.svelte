<script lang="ts">
	let { label }: { label: string } = $props();
</script>

{#snippet row<T>(value: T, prefix: string = '>')}
	<li>{prefix}{value}</li>
{/snippet}

<ul>
	{@render row(label)}
</ul>
