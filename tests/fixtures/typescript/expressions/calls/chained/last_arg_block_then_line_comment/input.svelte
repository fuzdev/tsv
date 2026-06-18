<script lang="ts">
	// The last argument of a chained call, carrying a block comment before the comma
	// and a line comment after it: the block trails the argument (before the trailing
	// comma), the line comment follows. The block is not relocated past the comma.
	foo
		.bar(
			x,
			a /* c1 */, // c2
		)
		.baz();

	// Single argument form.
	foo
		.bar(
			a /* c1 */, // c2
		)
		.baz();
</script>
