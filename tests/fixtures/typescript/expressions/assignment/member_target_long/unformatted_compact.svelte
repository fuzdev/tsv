<script>
	// An assignment target's `.prop` lookups never break — prettier's `printMemberExpression`
	// inlines every lookup when the assignment's left is not a plain identifier
	// (member.js `shouldInline`), so the whole target prints as one unbreakable unit.
	// What the rule measures is the target's own head, `<target> <operator>`.

	// head exactly 100 — the value drops to its own line
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.cccccccccccccccccccccccccccccccccc=eee;

	// head 101 — the target has no break point, so it stays intact and overflows
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.ccccccccccccccccccccccccccccccccccc=eee;

	// Contrast: a computed lookup IS a break point, so an over-width head sheds width
	// inside its own brackets
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb['cccccccccccccccccccccccccccccccc']=eee;

	// The target has nowhere to break, so an unbreakable value stays welded to the
	// operator: 100 chars, then 101 chars overflowing rather than hanging the value
	aaa.bbb.ccc+=`xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`;
	aaa.bbb.ccc+=`xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`;

	// Control: a target and value that fit
	a.b.c=d;
</script>
