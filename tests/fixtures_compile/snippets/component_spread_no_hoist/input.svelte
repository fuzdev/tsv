<script>
	import Foo from './Foo.svelte';
	let n = $state({ a: 1 });
</script>

{#snippet s()}<Foo {...n} />{/snippet}
{@render s()}
