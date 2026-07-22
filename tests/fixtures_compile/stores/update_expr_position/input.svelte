<script>
	import { count } from './stores.js';
	function next() {
		let y = $count++;
		return y;
	}
</script>

<button onclick={next}>{$count}</button>
