<script>
	import { count } from './stores.js';
	function set_five() {
		$count = 5;
	}
</script>

<button onclick={set_five}>{$count}</button>
