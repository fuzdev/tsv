<script>
	// Two block comments the author left byte-adjacent are ONE comment, so the merged
	// text is what decides whether the parens the run leads are a cast. The `@type`
	// marker may therefore sit anywhere in the run, not only in the comment the `(`
	// follows.

	// the marker in the head of a pair
	const a = /** @type {A}
	 *//** c
	 */ (x);

	// ...in the head of a run of three
	const b = /** @type {A}
	 *//** c1
	 *//** c2
	 */ (x);

	// ...in the middle of one
	const c = /** c1
	 *//** @type {A}
	 *//** c2
	 */ (x);

	// ...and in the tail, the reading the `(` alone already gives
	const d = /** c
	 *//** @type {A}
	 */ (x);

	// `@satisfies` marks a cast the same way
	const e = /** @satisfies {A}
	 *//** c
	 */ (x);

	// The position of the parens does not enter into it - a call argument
	fn(
		/** @type {A}
	 *//** c
	 */ (x)
	);

	// ...and a member base.
	const f = /** @type {A}
	 *//** c
	 */ (x).y;

	// Contrast: one byte of separator and the two are two comments again, so the
	// trailing one alone decides - no marker there, no cast, parens stripped.
	const g = /** @type {A}
	 */ /** c
	 */ x;

	// Contrast: byte-adjacent but not both indentable, which is the other half of the
	// nestling rule - again two comments, again no cast.
	const h = /*@type {A}*/ /*c*/ x;

	// Contrast: a nestled run carrying no marker at all.
	const i = /** c1
	 *//** c2
	 */ x;
</script>
