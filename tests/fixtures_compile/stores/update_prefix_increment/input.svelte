<script>
	import { count } from './stores.js';
	function inc() {
		++$count;
	}
</script>

<button onclick={inc}>{$count}</button>
