<script lang="ts">
	// A multiline block comment GLUED to the body (the body opens on the comment's
	// closing line) does not by itself break after `=>` — whether the body hugs is
	// the body's own question, exactly as it is without a comment.

	// object body (its parens are required, so they are kept): hugs
	const a = (x) => /* line1
	line2 */ ({ b: 1 });

	// array body: hugs
	const b = (x) => /* line1
	line2 */ [1, 2];

	// the same object body as a call argument (the expand-last hug path)
	fn1((x) => /* line1
	line2 */ ({ b: 1 }));

	// a body that breaks stays hugged too
	const c = (x) => /* line1
	line2 */ ({
		d: 1,
		e: 2
	});

	// call body: opens below `=>` — the body's kind decides, not the comment
	const f = (x) =>
		/* line1
	line2 */ fn1({ b: 1 });

	// identifier body: same
	const g = (x) =>
		/* line1
	line2 */ h;

	// Contrast: a newline AFTER the comment breaks even a hugging body
	const i = (x) =>
		/* line1
		line2 */
		({ j: 1 });
</script>
