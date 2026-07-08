<script lang="ts">
	// Short — whole statement fits inline, no break
	const short = !(aa.bb > cc.dd);

	// 100 boundary — the whole `const … = !(…);` line is exactly 100, stays inline
	const boundary = !(aaaaaaaaaaaaaaaaaaaa.bbbbbbbbbb > cccccccccccccccccccccccccccc.dddddddddddddd);

	// Over 100 — relational too long: parens expand, the 2-operand binary sits on its own line
	const overflow = !(
		aaaaaaaaaaaaaaaaaaaa.bbbbbbbbbb > cccccccccccccccccccccccccccc.dddddddddddddddd
	);

	// Arithmetic chain — 3 operands break at each `+` inside the expanded parens
	const arithmetic = !(
		aaaaaaaaaaaaaaaaaaaa.bbbbbbbbbb +
		cccccccccccccccc.dddddddddd +
		eeeeee.fffff
	);

	// Prefix minus — the paren expansion applies to any unary operator, not just `!`
	const negate = -(
		aaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbb + ccccccccccccccccccccccccccc.dddddddddddddd
	);

	// Equality in a return — same paren expansion at the deeper indent
	function fn() {
		return !(
			aaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbb ===
			cccccccccccccccccccccc.dddddddddd + eeeeeeee.ff
		);
	}
</script>
