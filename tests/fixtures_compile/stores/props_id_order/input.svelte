<script>
	import { count } from './stores.js';
	const id = $props.id();
</script>

<p {id}>{$count}</p>
