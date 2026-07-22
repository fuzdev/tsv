<script>
	import { count } from './stores.js';
	function scale() {
		$count *= 2;
	}
</script>

<button onclick={scale}>{$count}</button>
