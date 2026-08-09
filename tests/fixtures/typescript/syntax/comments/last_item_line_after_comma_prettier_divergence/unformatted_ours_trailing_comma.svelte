<script lang="ts">
	// A line comment after the LAST item's trailing comma keeps the line the author
	// gave it: the comma is deleted (trailingComma: 'none'), so the anchor that pulls
	// an after-comma comment up onto the item's line is not in the output at all
	function fn(
		a: A,
		b: B
		, // c
	) {}

	// Arrow parameters
	const fn2 = (
		a: A,
		b: B
		, // c
	) => {};

	// Function type parameters
	type F = (
		a: A,
		b: B
		, // c
	) => void;

	// Tuple elements
	type T = [
		A,
		B
		, // c
	];

	// Type parameters
	function fn3<
		A,
		B
		, // c
	>() {}
</script>
