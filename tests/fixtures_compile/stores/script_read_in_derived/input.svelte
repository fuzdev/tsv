<script>
	import { count } from './stores.js';
	let doubled = $derived($count * 2);
</script>

<p>{doubled}</p>
