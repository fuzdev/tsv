<script>
	function f() {
		// argument tail: the pre-`;` own-line comment lands past the `;`, at the `if`'s level
		if (a) return x;
		// c1
		// c2
		else fn1();
	}
	// throw consequent: the same gap through the argument form
	if (b) throw y;
	// c3
	else fn2();
	// an assignment operand keeps its clarity parens; the comment still lands past the `;`
	if (c) throw (x = y);
	// c4
	else fn3();
	// a binary operand: the conditional-paren arm's terminator gap
	if (d) throw x + y;
	// c5
	else fn4();
	// the hanging-paren form: the `)`→`;` gap comment lands past the statement
	if (e)
		throw (
			// lead
			x
		);
	// c6
	else fn5();
	// a collapsed clause chain: the comment lands at the statement list's level
	loop: for (;;) if (g) throw x;
	// c7
	next();
	// an author blank above the comment survives past the `;`
	if (h) throw w;

	// c8
	else fn6();
</script>
