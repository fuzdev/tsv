<script>
	import { w } from './s.js';
</script>

<svelte:boundary>
	<p>{$w}</p>
	{#snippet pending()}<i>q</i>{/snippet}
</svelte:boundary>
