<script lang="ts">
	// First-argument leading comment with a blank line before the argument, on a
	// member-chain call: the blank is preserved (matches prettier).

	// line comment
	obj
		.method(
			// c

			a
		)
		.other();

	// block comment
	obj
		.method(
			/* c */

			b
		)
		.other();
</script>
