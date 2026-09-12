<script lang="ts">
	// A frozen `for` init clause prints verbatim, so the ambient `[~In]` pair cannot be
	// placed at the `in`'s own position the way the unfrozen clause places it — the one
	// pair goes around the whole slice. The question is therefore the SLICE's: the clause
	// root need not be the `in` binary, only reach one through a position the grammar
	// threads `[~In]` into.

	// a sequence operand
	for (
		// prettier-ignore
		(aaa1, 'aaa'  in  bbb);
		;
	) {}

	// an assignment value
	for (
		// prettier-ignore
		(ccc = 'aaa'  in  bbb);
		;
	) {}

	// a compound assignment's value
	for (
		// prettier-ignore
		(ccc += 'aaa'  in  bbb);
		;
	) {}

	// a conditional's alternate
	for (
		// prettier-ignore
		(ooo ? ppp : 'aaa'  in  bbb);
		;
	) {}

	// a conditional's test
	for (
		// prettier-ignore
		('aaa'  in  bbb ? ppp : qqq);
		;
	) {}

	// a logical operand
	for (
		// prettier-ignore
		(qqq && 'aaa'  in  bbb);
		;
	) {}

	// an arrow's concise body
	for (
		// prettier-ignore
		(() => 'aaa'  in  bbb);
		;
	) {}

	// An arrow's concise body is `[?In]` too, and the freeze there is the arrow's own
	// `=>`→body head rather than the clause's. The root case takes the pair from the
	// ARROW-BODY position; a descended one takes it from the slice.
	for (
		xxx = () =>
			// prettier-ignore
			('aaa'  in  bbb);
		;
	) {}

	for (
		xxx = () =>
			// prettier-ignore
			(ccc || 'aaa'  in  bbb);
		;
	) {}

	// an `as` / `satisfies` operand — the cast is over the binary, so the `in` is not the root
	for (
		// prettier-ignore
		('aaa'  in  bbb as any);
		;
	) {}

	for (
		// prettier-ignore
		('aaa'  in  bbb satisfies any);
		;
	) {}

	// a `yield` / `yield*` argument
	function* gen() {
		for (
			// prettier-ignore
			(yield 'aaa'  in  bbb);
			;
		) {}

		for (
			// prettier-ignore
			(yield* 'aaa'  in  bbb);
			;
		) {}
	}

	// Controls. An `in` the author already parenthesized rides inside the slice with its
	// own pair, and a conditional's CONSEQUENT is `[+In]` whatever the conditional is, so
	// neither needs one.
	for (
		// prettier-ignore
		aaa1, ('aaa'  in  bbb);
		;
	) {}

	for (
		// prettier-ignore
		ooo ? 'aaa'  in  bbb : qqq;
		;
	) {}
</script>
