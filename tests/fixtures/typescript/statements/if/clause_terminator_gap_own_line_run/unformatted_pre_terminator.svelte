<script>
	function f() {
		// bare return: the pre-`;` own-line comment keeps its line, at the `if`'s level
		if (a) return
		// c1
		; // c2
		else fn1();
		// a collapsed clause chain: the comment lands at the statement list's level
		loop: for (;;) if (b) break loop
		// c3
		;
	}
	// declaration consequent: the same gap through the `var` tail
	if (c) var v = 1
	// c4
	;
	else fn2();
	// no else: the comment lands past the whole statement
	if (d) expr1
	// c5
	;
	next();
	// a label is transparent: its body's tail is the label's tail
	if (e) lbl: expr2
	// c6
	;
	else fn3();
	// an author blank above the comment survives past the `;`
	if (g) expr3

	// c7
	;
	else fn4();
</script>
