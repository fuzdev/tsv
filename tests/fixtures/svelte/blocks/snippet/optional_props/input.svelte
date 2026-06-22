<script lang="ts">
	import type { Snippet } from 'svelte';
	let { children, header = fallback }: { children?: Snippet; header?: Snippet } = $props();
</script>

{#snippet fallback()}
	<span>default</span>
{/snippet}

{@render header()}
{@render children?.()}
