<script lang="ts">
	// A block comment ending a redundant pair inside an operand that takes the clarity pair
	// the printer adds around a mixed-precedence operand trails that clarity pair.
	a >> (b /* c1 */) >> d;

	x = a - (b /* c2 */) << d;

	const y = a % (b /* c3 */) * d;

	function f1() {
		return a >> (b /* c4 */) >> d;
	}

	f(g(a >> (b /* c5 */) >> d));

	o = { k: a >> (b /* c6 */) >> d };

	for (a ?? b && (c /* c7 */); ; ) {}

	// The pair written around the whole operand: the comment already trails it.
	z = (a - b /* c8 */) >> 1;

	// A right-nested power: the comment leaves the statement.
	w = a ** b ** (c /* c9 */);

	// A comment written after the author's pair stays behind the moved one, in source order.
	a >> (b /* c11 */) /* e */ >> d;

	// A statement header's test.
	if (a >> (b /* c12 */) >> d) {
	}

	// A multi-line block trails the pair as well: a line break ahead of an operator is harmless.
	v = a >> (b /* c13
	 */) >> d;

	// The author's pair around a whole statement strips too: the comment trails the statement.
	(a >> (b /* c14 */));
</script>

<p>{a >> (b /* c10 */) >> d}</p>
