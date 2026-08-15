<script>
	// Else - non-block body, an own-line run above an OWNED block comment that
	// carries a blank line of its own
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

	// Labeled - the `:` gap's frozen body is the third caller of the same scan
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
