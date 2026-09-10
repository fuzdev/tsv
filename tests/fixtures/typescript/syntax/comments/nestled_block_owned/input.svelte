<script>
	// A nestled pair takes the TRAILING comment's ownership, because that is the comment
	// whose `*/` binds the token after it. So the merged comment is printed by the node its
	// token begins, and a paren the printer synthesizes around an ENCLOSING expression
	// lands outside the pair rather than between the comment and what it leads.

	// A `**` operand.
	const a =
		/** a
		 *//** b
		 */ c ** 2;

	// A member base.
	const d =
		/** a
		 *//** b
		 */ e.f.g;

	// A `new` callee.
	const h = new /** a
	 *//** b
	 */ I();

	// A `??` operand.
	const j =
		/** a
		 *//** b
		 */ k ?? l;

	// A call argument, where the pair leads the argument rather than the whole call.
	fn(
		/** a
		 *//** b
		 */ m
	);

	// A bundler annotation closing the pair still marks the call after it.
	const n =
		/** a
		 *//* @__PURE__
		 */ fn();

	// Control - one byte of gap, and the two comments are owned separately.
	const o =
		/** a
		 */ /** b
		 */ p ?? q;
</script>
