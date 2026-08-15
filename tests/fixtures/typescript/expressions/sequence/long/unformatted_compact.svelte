<script>
	/* 100 chars - stays inline */
	(expr_aaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbb, expr_cccccccccccccccccccccccccc);

	/* 101 chars - breaks; operands after the first indent one level */
	(expr_aaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbb, expr_ccccccccccccccccccccccccccc);

	/* First operand breaking internally - its own lines stay at the statement's indent, only
	   the operands after it take the continuation indent */
	((a = cond ? consequent_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa : alternate_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb), b);

	/* Value position - operands align with the first, no extra indent */
	const a = (expr_aaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbb, expr_cccccccccccccccccccccccccccccccccc);

	/* Arrow body - operands hang inside the parens, closing paren on its own line */
	const fn = () => (expr_aaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbb, expr_cccccccccccccccccccc);

	/* Return argument - same hanging form as the arrow body */
	function fn2() {
		return (expr_aaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbb, expr_cccccccccccccccccccc);
	}

	/* For-head update clause - no parens; operands after the first indent one level */
	for (let i = 0; i < n; expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa++, expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb++, expr_cccccccccccccccccccccccccccccc++) {}
</script>
