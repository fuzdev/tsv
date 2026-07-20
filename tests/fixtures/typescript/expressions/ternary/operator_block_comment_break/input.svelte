<script>
	// Broken ternary: a block comment after `:` keeps its authored break; the value
	// stays on its own line at the branch indent
	const a = condAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? consequentBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: /* c */
			alternateCcccccccc;

	// Same on the `?` branch
	const b = condAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? /* c */
			consequentBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: alternateCcccccccc;

	// Glued control: a comment the author kept on the value's line stays glued
	const c = condAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? consequentBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: /* c */ alternateCcccccccc;

	// JSDoc @param before an arrow alternate: broken by the multiline body, not width
	const d = fn.prop
		? fn.prop
		: /** @param {string} a */
			(a) => {
				return fn.parse(a).b;
			};

	// Chained: a comment on the middle branch keeps its break
	const e = condAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? consequentBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: /* c */
			condDddddddddddddddddddddddddddddddddddd
			? consequentEeeeeeeeeeeeeeeeeeeeeeeeeeee
			: alternateFff;

	// A multiline block comment keeps its break; a glued multiline block keeps its value glued
	const f = condAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? /* c1
			 c2 */ consequentBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: /* c1
			 c2 */
			alternateCcccccccc;

	// Two comments: each picks its own separator from the source after it
	const g = condAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? consequentBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: /* c1 */
			/* c2 */ alternateCcccccccc;

	// Nested consequent behind a breaking comment: chain breaks, parens drop
	const h = condAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? /* c */
			condDddddddddddddddddddddddddddddddddddd
			? consequentEeeeeeeeeeeeeeeeeeeeeeeeeeee
			: alternateFff
		: alternateCcccccccc;
</script>
