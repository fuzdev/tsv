<script>
	class Counter {
		count = $state(/* initial */ 0);
		label = $state(/** @type {string | null} */ (null));
		pending = $state(/* none yet */);
	}
	const counter = new Counter();
</script>

<p>{counter.count}{counter.label}{counter.pending}</p>
