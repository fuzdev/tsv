<script lang="ts">
	class Counter {
		count = $state(0);
		raw = $state.raw({nested: 'value'});
		double = $derived(this.count * 2);
		triple = $derived.by(() => this.count * 3);
		increment() {
			this.count++;
		}
	}
</script>
