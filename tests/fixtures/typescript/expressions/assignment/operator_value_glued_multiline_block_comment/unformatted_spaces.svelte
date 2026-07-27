<script lang="ts">
	// A multiline block comment glued to its value (the value sits on the comment's
	// closing line) collapses inline, matching prettier — it does NOT break after the
	// `=` / `:`. Covers the shared operator -> value gaps of the assignment layout.

	// variable declarator
	const a =
		/* line1
	line2 */ x;

	// assignment expression
	let b;
	b =
		/* line1
	line2 */ x;

	// object property
	const c = {
		k:
			/* line1
		line2 */ x
	};

	// class field
	class C {
		d =
			/* line1
		line2 */ x;
	}

	// Contrast: a newline after the comment hangs the value under the operator
	const e =
		/* line1
		line2 */
		x;
</script>
