<script>
	// the hoisted block comment leads the next statement, as it does in a plain list
	if (cond) expr1;
	/* c1 */ fn1();
	// the same gap under a `while` head
	while (cond) expr2;
	/* c2 */ fn2();
	// a labeled loop's collapsed clause
	loop: for (;;) break loop;
	/* c3 */ fn3();
	// an `else` clause body's own tail
	if (a) fn4();
	else expr3;
	/* c4 */ fn5();
	// the remaining clause heads (`with` shares this arm too, but is a strict-mode error
	// and so unreachable from a module `<script>`)
	for (const key in expr4) expr5;
	/* c5 */ fn6();
	for (const item of expr6) expr7;
	/* c6 */ fn7();
	// an author blank below the terminator rides ABOVE the glued run, which keeps no
	// line below itself to carry one
	if (cond) expr10;

	/* c8 */ fn9();
	// CONTROL: nothing follows in the block, so the comment keeps its own line
	function f() {
		if (cond) expr11;
		/* c9 */
	}
</script>
