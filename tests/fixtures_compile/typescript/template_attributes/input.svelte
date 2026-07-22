<script lang="ts">
	import Child from './Child.svelte';

	let { title, extra }: { title: string; extra: Record<string, unknown> } = $props();
</script>

<div title={title as string} data-x="a {title as string} b">text</div>
<Child name={title as string} {...extra as Record<string, string>} />
