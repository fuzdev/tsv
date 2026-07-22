<script>
	import { count } from './stores.js';
	function total() {
		return $count + 1;
	}
</script>

<p>{total()}</p>
