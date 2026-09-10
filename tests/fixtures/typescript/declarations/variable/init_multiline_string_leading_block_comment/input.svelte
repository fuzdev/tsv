<script lang="ts">
	// A line-continuation string forces the declarator's mandatory break after `=`, and
	// that arm builds the value itself rather than routing through the assignment layout.
	// A block comment leading the value still has to be printed: an OWNED one (glued to
	// the value's first token) is hoisted out of the value's doc, so this arm is the one
	// that emits it — below the `=`, where the author glued it.

	// indentable kind
	const a1 =
		/**
		 * c
		 */ 'a \
b';

	// preserved kind
	const a2 =
		/* c
d */ 'a \
b';

	// single-line block: nothing is hoisted, and the gap's own emitter lands it in the
	// same place — so the two paths agree on where the run goes
	const a3 =
		/* c */ 'a \
b';

	// null control: no comment
	const a4 =
		'a \
b';

	// null control: a single-line string takes the ordinary layout
	const a5 =
		/**
		 * c
		 */ 'ab';

	export { a1, a2, a3, a4, a5 };
</script>
