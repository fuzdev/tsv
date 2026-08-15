<script>
	// A blank-line scan must not read an OWNED comment's own interior newlines as an
	// author blank. Every gap below ends at an expression start, so a block comment
	// the author glued to it is owned - inside the gap, printed by the value's doc,
	// and skipped by the run the gap emits.

	// Else - non-block body, an own-line run above the owned comment
	if (a) fn1();
	else
		// c1
		/* owned

	*/ fn2();

	// Else - the same position with a real author blank (null control)
	if (a) fn1();
	else
		// c1

		/* owned

	*/ fn2();

	// While - the `)` twin, which reaches the answer through the leading-run
	// emitter rather than this gap's tail (control)
	while (a)
		// c1
		/* owned

	*/ fn2();

	// Binary operator - the run trails the operator, the owned comment leads the
	// operand
	const b =
		xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx + // c1
		/* owned

		*/ yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy;

	// Binary operator - the same comment with no interior blank (null control; a
	// real author blank there is the sanctioned
	// `binary/operator_trailing_comment_blank` divergence, so it is pinned separately)
	const c =
		xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx + // c1
		/* owned
		 */ yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy;

	// Ternary `?` - the branch is the expression start, and the fabricated blank
	// would also drop the branch onto its own line
	const d = cond
		? // c1
			/* owned

			*/ value1
		: value2;

	// Ternary `?` - a real author blank there survives (null control)
	const e = cond
		? // c1

			/* owned */ value1
		: value2;

	// Labeled - the `:` gap's frozen body is another caller of the same scan
	lll:
	// prettier-ignore
	/* owned

	*/ fn3(  bbb  );

	// Labeled - a real author blank there survives (null control)
	mmm:
	// prettier-ignore

	/* owned

	*/ fn3(  bbb  );
</script>
