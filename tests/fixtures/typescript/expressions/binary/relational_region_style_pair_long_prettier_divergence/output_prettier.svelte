<script lang="ts">
	// At 100 columns the line holds with the arrow's conditional body bare; at 101 the list breaks
	fn(
		lower < upper,
		(value) => (value ? xxxxxxxxxxxxxxxxxxx : fallback),
		limit > (lower + upper) / 2
	);
	fn(
		lower < upper,
		(value) => (value ? xxxxxxxxxxxxxxxxxxxx : fallback),
		limit > (lower + upper) / 2
	);
	// The same boundary for an assignment
	fn(
		lower < upper,
		(current = xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx),
		limit > (lower + upper) / 2
	);
	fn(
		lower < upper,
		(current = xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx),
		limit > (lower + upper) / 2
	);
	// The arrow as the last element, holding the closer in its body
	fn(lower < upper, (value) =>
		value ? xxxxxxxxxxxxxxxxxxxxxxxxxx : fallback > (lower + upper) / 2
	);
	fn(lower < upper, (value) =>
		value ? xxxxxxxxxxxxxxxxxxxxxxxxxxx : fallback > (lower + upper) / 2
	);
	// In a broken list, the arrow's own line holds at 100 columns and its body hangs at 101
	fn(
		lower < upper,
		(value) =>
			value ? xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx : fallback,
		limit > (lower + upper) / 2
	);
	fn(
		lower < upper,
		(value) =>
			value ? xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx : fallback,
		limit > (lower + upper) / 2
	);
</script>
