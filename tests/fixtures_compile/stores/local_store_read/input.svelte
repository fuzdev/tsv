<script>
	import { writable } from 'svelte/store';
	let count = writable(0);
	let doubled = $count * 2;
</script>

<p>{doubled}</p>
