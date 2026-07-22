<script>
	import { get_thing } from './thing.js';
	let b = $derived(get_thing());
	// after unthunk
	let c = 3;
</script>

<p>{b}{c}</p>
