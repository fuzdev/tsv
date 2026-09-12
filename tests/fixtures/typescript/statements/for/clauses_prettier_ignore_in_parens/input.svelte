<script lang="ts">
	// An `in` binary lexically under a `for` header's init is parenthesized so the header
	// does not read as a for-in head. Those parens are the printer's, so a freeze does not
	// take them away: they ride outside the verbatim slice, as every other required pair
	// does.
	for (
		// prettier-ignore
		('aaa'  in  bbb);
		;
	) {}

	// The same rule one binding in — an assignment RHS, a compound RHS and a declarator
	// initializer, each with the `in` frozen.
	for (
		ccc =
			// prettier-ignore
			('aaa'  in  bbb);
		;
	) {}

	for (
		ddd +=
			// prettier-ignore
			('aaa'  in  bbb);
		;
	) {}

	for (
		let eee =
			// prettier-ignore
			('aaa'  in  bbb);
		;
	) {}

	// A position that already routes through the paren rule asks it ONCE: a call argument
	// and an array element are `[+In]` in their own right, so the pair they print is the
	// ambient rule's, and nothing may wrap it a second time.
	for (
		ccc = fn(
			// prettier-ignore
			('aaa'  in  bbb)
		);
		;
	) {}

	for (
		ccc = [
			// prettier-ignore
			('aaa'  in  bbb)
		];
		;
	) {}

	// A sequence operand is lexically under the init too, so it takes the pair as well.
	for (
		fff,
			// prettier-ignore
			('aaa'  in  bbb),
			ggg;
		;
	) {}
</script>
