<script>
	// An earlier call takes a callback, so a breaking last argument keeps the chain
	// expanded rather than hugging it onto the flat chain (prettier's
	// `lastGroupWillBreakAndOtherCallsHaveFunctionArguments`): object argument.
	const a1 = expr.filter((x) => x.a).fn1({ prop1: 'value1value1value1value1', prop2: 'value2value2value2value2', prop3: 'value3value3value3value3' });

	// Same refusal, array argument.
	const a2 = expr.filter((x) => x.a).fn1([{ prop1: 'value1value1value1value1', a: 1 }, { prop2: 'value2value2value2value2', a: 2 }, { prop3: 'value3value3value3value3', a: 3 }]);

	// Same refusal, the arrow's parenthesized object body.
	const a3 = expr.filter((x) => x.a).map((item) => ({ prop1: 'value1value1value1value1', prop2: 'value2value2value2value2', prop3: 'value3value3value3value3' }));

	// A function expression counts too, wherever it sits in the earlier call's list.
	const a4 = expr.fn2(a, function (b) { return b; }).fn1({ prop1: 'value1value1value1value1', prop2: 'value2value2value2value2', prop3: 'value3value3value3value3' });
</script>
