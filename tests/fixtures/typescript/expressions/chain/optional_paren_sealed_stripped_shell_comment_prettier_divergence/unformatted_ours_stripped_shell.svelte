<script lang="ts">
	// A comment in grouping parens the parser strips from around a sealed chain's member
	// prints once, in front of that member — where it lands without the seal
	// (`x.y /* c */.p`), and outside the pair that seals the chain.
	(/* c1 */ (x?.y).p).t();
	(/* c2 */ (x?.y).p).t().u();
	f((/* c3 */ (x?.y).p).t());
	function g() {
		return (/* c4 */ (x?.y).p).t();
	}

	// A `!` seal, a computed lookup, a deeper member, an IIFE callee, a chain a `!` ends, an
	// optional continuation, the other sealed bases, a multi-line block and the call
	// spellings take the same answer.
	(/* c5 */ (x?.y)!.p).t();
	(/* c6 */ (x?.y)[0]).t();
	(/* c7 */ (x?.y).p.q).t();
	(/* c8 */ (() => 1)().p).t();
	(/* c16 */ (x?.y).p).t()!;
	(/* c17 */ (x?.y).p)?.t();
	(/* c18 */ (x?.()).p).t();
	(/* c19 */ (x?.y!).p).t();
	(/** c20
	 */ (x?.y).p).t();
	(/* c21 */ (x?.y).p).t<T>();
	(/* c22 */ (x?.y).p).t!();

	// Nested shells each land in front of their own member.
	(/* c10 */ (/* c9 */ (x?.y).p).q).t();

	// A line comment glued to the `(`, and one on its own line.
	(// c11
	(x?.y).p).t();
	(
		// c12
		(x?.y).p
	).t();

	// Controls: with no member below the shell (no lookup, or a call), the comment leads.
	(/* c13 */ (x?.y).p);
	(/* c14 */ (x?.y)).t();
</script>

<p>{(/* c15 */ (x?.y).p).t()}</p>
