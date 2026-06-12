<script lang="ts">
	// Array as last arg with trailing comment - should stay expanded
	fn(
		a,
		b,
		['c'], // comment
	);

	// Object as last arg with trailing comment - should stay expanded
	fn(
		a,
		b,
		{c}, // comment
	);

	// Multiple arrays with trailing comment on last
	fn(
		['a'],
		['b'],
		['c'], // comment
	);

	// Mixed types, array last with comment
	fn(
		'a',
		b,
		['c'], // comment
	);

	// Single arg call - array with comment
	fn(
		['a'], // comment
	);

	// Type assertion with trailing comment
	fn(
		a,
		['b'] as const, // comment
	);

	// Satisfies with trailing comment
	fn(
		a,
		['b'] satisfies string[], // comment
	);

	// New expression with trailing comment
	new Class(
		a,
		['b'], // comment
	);

	// Empty array with comment
	fn(
		a,
		[], // comment
	);

	// Empty object with comment
	fn(
		a,
		{}, // comment
	);

	// Nested array with comment
	fn(
		a,
		[['nested']], // comment
	);
</script>
