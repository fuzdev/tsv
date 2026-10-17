<script lang="ts">
	// At 100 columns the line holds with the pair behind `typeof`; at 101 the list breaks
	fn(lower < upper, typeof (xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx), limit > (lower + upper) / 2);
	fn(
		lower < upper,
		typeof (xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx),
		limit > (lower + upper) / 2
	);
	// The same boundary behind a unary `-`
	fn(lower < upper, -(1), limit > (lower + upper) / xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx);
	fn(
		lower < upper,
		-(1),
		limit > (lower + upper) / xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
	);
</script>
