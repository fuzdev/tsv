<script lang="ts">
	// Leading comment on the first parameter
	fn((/* c1 */ a, b) => call(a));

	// Trailing comment on a parameter, before the comma
	fn((a /* c2 */, b) => call(a));

	// Leading comment on a later parameter
	fn((a, /* c3 */ b) => call(a));

	// Trailing comment on the last parameter, before the `)`
	fn((a, b /* c4 */) => call(a));

	// A `new` expression's arguments take the same path
	new Comp((/* c5 */ a) => call(a));

	// A member chain's arguments take the same path
	arr.map((/* c6 */ a, b) => call(a));
</script>
