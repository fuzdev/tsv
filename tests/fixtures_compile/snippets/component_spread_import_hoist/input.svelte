<script>
	import Foo from './Foo.svelte';
	import { cfg } from './cfg.js';
</script>

{#snippet s()}<Foo {...cfg} />{/snippet}
{@render s()}
