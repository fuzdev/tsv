<script lang="ts">
	// A comment in grouping parens the parser strips from around a sealed chain's member
	// prints once, in front of that member — where it lands without the seal
	// (`x.y /* c */.p`), and outside the pair that seals the chain.
	(x?.y) /* c1 */.p
		.t();
	(x?.y) /* c2 */.p
		.t()
		.u();
	f(
		(x?.y) /* c3 */.p
			.t()
	);
	function g() {
		return (x?.y) /* c4 */.p
			.t();
	}

	// A `!` seal, a computed lookup, a deeper member, an IIFE callee, a chain a `!` ends, an
	// optional continuation, the other sealed bases, a multi-line block and the call
	// spellings take the same answer.
	(x?.y)! /* c5 */.p
		.t();
	(x?.y) /* c6 */[0]
		.t();
	(x?.y).p /* c7 */.q
		.t();
	(() => 1)() /* c8 */.p
		.t();
	(x?.y) /* c16 */.p
		.t()!;
	(x?.y) /* c17 */.p
		?.t();
	(x?.()) /* c18 */.p
		.t();
	(x?.y!) /* c19 */.p
		.t();
	(x?.y) /** c20
	 */.p
		.t();
	(x?.y) /* c21 */.p
		.t<T>();
	(x?.y) /* c22 */.p
		.t!();

	// Nested shells each land in front of their own member.
	(x?.y) /* c9 */.p /* c10 */.q
		.t();

	// A line comment glued to the `(`, and one on its own line.
	(x?.y) // c11
		.p.t();
	(x?.y)
		// c12
		.p.t();

	// Controls: with no member below the shell (no lookup, or a call), the comment leads.
	/* c13 */ (x?.y).p;
	(/* c14 */ x?.y).t();
</script>

<p>
	{(x?.y) /* c15 */.p
		.t()}
</p>
