<script>
	import Child from './Child.svelte';
	// forces the wrapper
	let d = new Date();
</script>

<Child year={d.getFullYear()} />
