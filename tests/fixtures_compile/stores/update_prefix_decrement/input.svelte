<script>
	import { count } from './stores.js';
	function dec() {
		--$count;
	}
</script>

<button onclick={dec}>{$count}</button>
