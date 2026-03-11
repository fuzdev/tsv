<script lang="ts">
	import type {A} from './a';
	let a: A;
</script>

{#each Object.entries(a as Record<string, unknown>) as [key, value] (key)}
	<div>{key}: {value}</div>
{/each}
