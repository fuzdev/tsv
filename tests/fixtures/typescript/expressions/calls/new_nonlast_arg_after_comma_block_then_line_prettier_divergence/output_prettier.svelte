<script lang="ts">
	// A non-last `new` argument with a block comment after the comma and a line
	// comment after it: the block stays on the comma line, the line comment trails
	// the comma.
	new A(
		a /* c1 */, // c2
		b
	);

	// A blank line after the commented arg routes through the blank-line args path;
	// the after-comma block still stays on the comma line there too.
	new A(
		a /* c1 */, // c2

		b
	);
</script>
