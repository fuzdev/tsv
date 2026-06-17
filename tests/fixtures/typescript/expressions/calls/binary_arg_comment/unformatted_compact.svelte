<script>
	// continuation comment forces the break; operands after the first indent one level
	const a = fn(
		'a1' +
		// c
		'a2' +
		'a3',
	);

	// leading comment on the first operand alone does not break the chain
	const b = fn(
		// c
		'b1' + 'b2' + 'b3',
	);

	// leading comment on the first operand plus a continuation comment:
	// the continuation still indents one level under the first operand
	const c = fn(
		// c1
		'c1' +
		// c2
		'c2' +
		'c3',
	);

	// inline block comment leads the first operand; the chain breaks on width and
	// the continuation indents one level (block comments stay inline, do not break)
	const d = fn(
		/* c */ 'dddddddddddddddddddddddddddddddddddddddddddd' +
		'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee' +
		'fffffffffffffffffffffffffffffffffffffffffff',
	);
</script>
