<script>
	// Poorly breakable chains: "short arg" types → break at `=` when line > print_width
	// Prettier ref: isLoneShortArgument (utils/index.js:434)

	// Unary: signed numeric — short via isSignedNumericLiteral + UnaryExpression recursion
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaa = VeryLongClassName.createSomethingWithAVeryLongMethodNameHere(-42);

	// Unary: logical not — short via UnaryExpression recursion to Identifier
	const bbbbbbbbbbbbbbbbbbbbbbbbbbbb = VeryLongClassName.createSomethingWithAVeryLongMethodNameHere(!flag);

	// Unary: typeof — NOT short (typeof is not in isLoneShortArgument)
	const cccccccccccccccccccccccccccc = VeryLongClassName.createSomethingWithAVeryLongMethodNameHere(typeof val);

	// Unary: void — NOT short (void is not in isLoneShortArgument)
	const dddddddddddddddddddddddddddd = VeryLongClassName.createSomethingWithAVeryLongMethodNameHere(void 0);

	// RegExp: short pattern — short via regexpPattern.length <= threshold
	const eeeeeeeeeeeeeeeeeeeeeeeeeeee = VeryLongClassName.createSomethingWithAVeryLongMethodNameHere(/abc/g);

	// Zero-arg call: short callee — short via callee.name.length <= threshold - 2
	const ffffffffffffffffffffffffffff = VeryLongClassName.createSomethingWithAVeryLongMethodNameHere(foo());

	// Zero-arg call: long callee — NOT short (name exceeds threshold - 2 = 23)
	const gggggggggggggggggggggggggggg = VeryLongClassName.createSomethingWithAVeryLongMethodNameHere(veryLongFunctionNameHereXX());

	// Control: already-handled short types
	const hhhhhhhhhhhhhhhhhhhhhhhhhhhh = VeryLongClassName.createSomethingWithAVeryLongMethodNameHere('hi');
	const iiiiiiiiiiiiiiiiiiiiiiiiiiii = VeryLongClassName.createSomethingWithAVeryLongMethodNameHere(42);
	const jjjjjjjjjjjjjjjjjjjjjjjjjjjj = VeryLongClassName.createSomethingWithAVeryLongMethodNameHere(arg);
</script>
