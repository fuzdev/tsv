<script>
	// Bitwise XOR
	const xor = a^b;
	const xor2 = 1^2^3;

	// Bitwise NOT (unary)
	const not = ~ x;
	const notNot = ~ ~ x;

	// Left shift
	const left = a<<b;
	const left2 = 1<<4;

	// Right shift
	const right = a>>b;
	const right2 = 16>>2;

	// Unsigned right shift
	const unsigned = a>>>b;
	const unsigned2 = -1>>>0;

	// Exponentiation (right-associative)
	const exp = 2**3;
	const exp2 = 2**(3**2);

	// Mixed bitwise precedence (parens for clarity)
	const mixed = a|(b^(c&d));
	const mixed2 = ((a&b)^c)|d;

	// Same bitwise ops flatten (left parens removed, right kept)
	const same1 = a|b|c;
	const same2 = a&(b&c);
	const same3 = a^b^c;

	// Bitwise with comparison (parens for clarity)
	const compare = (a<b)&(c>d);

	// Shift with arithmetic
	const shiftAdd = (a<<2)+b;
	const shiftMul = a*(b>>1);
</script>
