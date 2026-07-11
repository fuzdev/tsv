<script>
	// Same-sign unary operand keeps parens — bare +(+x) is required, ++x is a different expression
	+(+x);
	-(-x);
	// Same-sign prefix-update operand keeps parens — +++x / ---x would re-tokenize
	+(++x);
	-(--x);
	// Nested sign operators — each level parenthesized
	+(+(+1));
	// In a binary right operand the inner chain still parenthesizes
	x + +(+(+1));
	// Different-sign operand needs no parens (no token merge)
	+-x;
	-+x;
	// Logical-not never merges
	!!x;
</script>
