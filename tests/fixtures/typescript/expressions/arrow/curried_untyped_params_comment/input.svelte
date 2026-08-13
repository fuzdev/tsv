<script>
	// A break-forcing comment in a head's parameter list breaks the chain after `=`, and every
	// head then sits at one indent level
	const lineComment =
		(x) =>
		(
			y // c
		) =>
			z;

	// The two other break-forcing kinds read the same way
	const ownLineBlock =
		(x) =>
		(
			y
			/* c */
		) =>
			z;
	const multilineBlock =
		(x) =>
		(
			y /* c1
	c2 */
		) =>
			z;

	// The first head's own parameter list is the same question
	const firstHead =
		(
			x // c
		) =>
		(y) =>
			z;

	// Any slot in the list: between two parameters, and leading the first
	const betweenParams =
		(
			a, // c
			b
		) =>
		(y) =>
			z;
	const leadingParam =
		(
			// c
			x
		) =>
		(y) =>
			z;

	// Three heads all align, whichever one carries the comment
	const threeHeads =
		(x) =>
		(
			y // c
		) =>
		(w) =>
			z;

	// An `async` head, a block-statement terminal and a hugging object terminal
	const asyncHead =
		async (x) =>
		(
			y // c
		) =>
			z;
	const blockBody =
		(x) =>
		(
			y // c
		) => {
			go();
		};
	const objectBody =
		(x) =>
		(
			y // c
		) => ({ a: 1 });

	// A binaryish operand indents its heads progressively instead of aligning them
	const binaryish =
		cond ||
		((x) =>
			(
				y // c
			) =>
				z);

	// Control: a single-line block glued to a parameter forces nothing - the chain stays flat
	const glued = (x /* c */) => (y) => z;

	// Control: no comment at all
	const plain = (x) => (y) => z;
</script>
