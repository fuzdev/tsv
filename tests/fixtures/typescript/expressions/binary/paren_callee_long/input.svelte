<script>
	// a short chain fits, so the parens stay welded to the argument list
	const r1 = (aa && bb)();

	// 100 chars - the callee still fits, the parens stay welded
	const r2 = (aaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbb && cccccccccccccccccccccccc)();

	// 101 chars - the parens expand onto their own lines and the operands break with them
	const r3 = (
		aaaaaaaaaaaaaaaaaaaaaaaaa &&
		bbbbbbbbbbbbbbbbbbbbbbbbb &&
		ccccccccccccccccccccccccc
	)();

	// an arithmetic chain takes the same shape
	const r4 = (
		aaaaaaaaaaaaaaaaaaaaaaaaaa +
		bbbbbbbbbbbbbbbbbbbbbbbbbb +
		ccccccccccccccccccccccccc
	)();

	// a two-operand chain keeps a group of its own, so it stays flat on the indented line
	const r5 = (
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	)();

	// a curried call is the same callee position
	const r6 = (
		aaaaaaaaaaaaaaaaaaaaaaaaa &&
		bbbbbbbbbbbbbbbbbbbbbbbb &&
		cccccccccccccccccccccccc
	)()();
</script>
