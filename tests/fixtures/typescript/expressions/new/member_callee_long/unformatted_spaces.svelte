<script>
	// A `new` callee's `.prop` lookups never break — prettier's `printMemberExpression`
	// inlines every lookup whose chain IS the callee (member.js
	// `shouldInlineNewExpressionCallee`), so the constructor name prints as one unbreakable
	// unit and the width falls to the argument list, or overflows.

	// 100 — fits inline
	new   aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.ccccccccccccccccccccccccccccc(   );

	// 101 — the callee has no break point, so it stays intact and overflows
	new   aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.cccccccccccccccccccccccccccccc(   );

	// Contrast: an argument list IS a break point, so the width sheds there
	new   aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.cccccccccccccccccccc(   dddd   ,   eeee   );

	// Contrast: a computed lookup sheds width inside its own brackets
	new   aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb[   'cccccccccccccccccccccccc'   ].dd(   );

	// Control: a callee that fits
	new   a.b.C(   );
</script>
