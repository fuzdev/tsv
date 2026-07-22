<script>
	class Counter {
		count = $state(0);
		increment() {
			this.count += 1;
		}
	}
	const c = new Counter();
</script>

<button onclick={() => c.increment()}>{c.count}</button>
