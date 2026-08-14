<script>
	// Bare initializer: the wide last-call argument fits on no chain line while the
	// chain's head fits, so the chain stays flat and the argument breaks inside (hug).
	const a1 = expr.fn1().map((item1) => ({ prop1: 'value1value1value1value1', prop2: 'value2value2value2value2', prop3: 'value3value3value3value3' }));

	// Call argument: the same hug.
	fn2(expr.fn1().map((item2) => ({ prop1: 'value1value1value1value1', prop2: 'value2value2value2value2', prop3: 'value3value3value3value3' })));

	// Property value: the same hug.
	fn3({ prop: expr.fn1().map((item3) => ({ prop1: 'value1value1value1value1', prop2: 'value2value2value2value2', prop3: 'value3value3value3value3' })) });

	// A direct object argument (no arrow): the same hug.
	const a4 = expr.fn1().fn4({ prop1: 'value1value1value1value1', prop2: 'value2value2value2value2', prop3: 'value3value3value3value3' });

	// The argument fits flat on the expanded chain line (exactly 100), so the chain
	// breaks and the argument stays flat.
	const a5 = expr.fn1().map((item5) => ({ prop1: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' }));

	// One char wider (101): the argument fits on no chain line, so the chain hugs.
	const a6 = expr.fn1().map((item6) => ({ prop1: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' }));
</script>

<!-- Template expression position: the same hug, under the tag's closing brace. -->
<p>{expr.fn1().map((item7) => ({ prop1: 'value1value1value1value1', prop2: 'value2value2value2value2', prop3: 'value3value3value3value3' }))}</p>
