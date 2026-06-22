<script lang="ts">
	// Spread before callback - hugs
	fn(...args, (x) => {
		return x;
	});

	// Spread of member expression before callback
	fn(...obj.items, (x) => {
		return x;
	});

	// Callback before spread - expands
	fn(
		(x) => {
			return x;
		},
		...args
	);

	// Multiple spreads before callback
	fn(...a, ...b, (x) => {
		return x;
	});
</script>
