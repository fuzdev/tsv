<script lang="ts">
	// Tuple element - the block stays after the comma, leading `U`
	type A = [T /* c */, U];

	// Type argument in type position
	type B = Foo<T /* c */, U>;

	// Type argument in call position - the same predicate feeds both
	const c = fn<T /* c */, U>();

	// Contrast - a comment before the comma trails the previous item
	type C = [T /* c */, U];
</script>
