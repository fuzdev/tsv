<script>
	import { writable } from 'svelte/store';
	// the store
	const count = writable(1);
	let doubled = /* read */ $count * 2; // doubled
	function inc() {
		// bump it
		$count += /* step */ 1;
		$count++; // again
		$count = /* reset */ 0;
	}
</script>

<button onclick={inc}>{doubled} {$count}</button>
