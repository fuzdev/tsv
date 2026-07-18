<script>
	import { count } from './stores.js';
	function bump() {
		$count += 1;
	}
</script>

<button onclick={bump}>{$count}</button>
