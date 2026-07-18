<script>
	import C from './C.svelte';
	let { a } = $props();
	let d = $derived(a * 2);
</script>

<C x={d + 1} />
