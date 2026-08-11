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

	// A glued RUN reaching the value collapses too — the multiline block may sit
	// at the head or the tail of the run.
	const f =
		/* line1
	line2 */ /* c1 */ x;
	const g =
		/* c2 */ /* line1
	line2 */ x;
	const h = {
		k:
			/* c3 */ /* line1
		line2 */ x
	};
	for (
		let i =
			/* c5 */ /* line1
	line2 */ 0;
		i < 2;
		i++
	) {
		fn1();
	}
	let j;
	j =
		/* line1
	line2 */ /* c6 */ x;
	class D {
		m =
			/* line1
		line2 */ /* c7 */ x;
	}
</script>
