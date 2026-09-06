<script lang="ts">
	// A sequence body whose first operand's grouping parens hold an own-line comment: the
	// sequence floats the comment out ahead of its own `(`, so the body no longer hugs the
	// `=>` — it breaks below it, the run and the sequence at the body's indent.
	const f1 = () => (
		// c
		a,
		b
	);
	const f2 = (x) => (
		// c
		a,
		b,
		c
	);

	// contrast: a comment-free sequence body hugs the arrow
	const f3 = () => (a, b);
</script>
