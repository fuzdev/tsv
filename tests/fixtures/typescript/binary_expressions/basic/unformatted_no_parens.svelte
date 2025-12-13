<script>
	// Arithmetic operators
	const add = 1 + 2;
	const sub = 5 - 3;
	const mul = 4 * 2;
	const div = 8 / 4;
	const mod = 7 % 3;

	// Comparison operators
	const lt = a < b;
	const gt = a > b;
	const lte = a <= b;
	const gte = a >= b;
	const eq = a == b;
	const neq = a != b;
	const seq = a === b;
	const sneq = a !== b;

	// Logical operators
	const and = a && b;
	const or = a || b;
	const nullish = a ?? b;

	// Bitwise operators
	const bitAnd = a & b;
	const bitOr = a | b;

	// Precedence tests (prettier adds parens for clarity when mixing && with ||)
	const prec1 = 1 + 2 * 3;
	const prec2 = a && b || c;
	const prec3 = a || b && c;
	const prec4 = a < b && c > d;

	// Chained equality (prettier adds parens)
	const eq1 = a == b == c;
	const eq2 = a === b === c;
	const eq3 = a != b != c;

	// Mixed multiplicative (prettier adds parens for different operators)
	const mult1 = a * b * c; // same operator - no parens
	const mult2 = a * b / c; // different operators - prettier adds parens
	const mult3 = a / b / c; // same operator - no parens

	// Modulo mixing (prettier adds parens when mixing with * or /)
	const mod1 = a * b % c; // * then % - prettier adds parens
	const mod3 = a % b * c; // % then * - prettier adds parens
	const mod4 = a / b % c; // / then % - prettier adds parens
	const mod5 = a % b / c; // % then / - prettier adds parens

	// Chained modulo (prettier adds parens for same operator - unlike *, /, +, -)
	const chainMod1 = a % b % c;
	const chainMod2 = a % b % c % d;
	const chainMod3 = a % b % c % d % e;

	// Modulo with additive operators (prettier adds parens for clarity)
	const modAdd1 = a % b + c;
	const modAdd2 = a + b % c;
	const modAdd3 = a % b - c;
	const modAdd4 = a - b % c;
	const modAddComplex = a * b % c + d; // mult, then mod, then add

	// Nullish coalescing with logical
	// Note: ?? has LOWER precedence than || and &&, so a ?? b || c parses as a ?? (b || c)
	// To get (a ?? b) || c, you must use explicit parens (prettier preserves them)
	const null1 = (a??b)||c; // explicit parens - prettier preserves
	const null2 = (a??b)&&c; // explicit parens - prettier preserves
	const null3 = a ?? b ?? c; // same operator - no parens

	// Multiple levels of same operator (no parens needed)
	const multi1 = a && b && c;
	const multi2 = a || b || c;
	const multi3 = a + b + c + d;

	// Complex nested logical (prettier adds parens)
	const complex1 = a && b || c && d;
	const complex2 = a || b && c || d;
	const complex3 = (a||b)&&(c||d); // parens required for this grouping

	// Bitwise operators
	const bit1 = a & b;
	const bit2 = a | b;

	// Arithmetic precedence chains (no parens needed - precedence is clear)
	const arith1 = a + b * c;
	const arith2 = a * b + c;
	const arith3 = a + b * c - d / e;
	const arith4 = a - b + c; // left-to-right, same precedence
	const arith5 = a / b * c; // different multiplicative ops - prettier adds parens

	// Arrow functions with binary expressions
	const fn1 = (a) => a + 1;
	const fn2 = (a, b) => a * b + 1;
	const fn3 = (x) => x > 0 && x < 10;

	// Unary + binary
	const neg = -a + b;
	const not = !a && b;

		// Unary + parenthesized (parens needed for grouping after unary operator)
	const notParen = !(a && b);
	const negParen = -(a + b);
	const negNested = -((a + b) * c);

	// Deeply nested parens (prettier removes unnecessary nesting)
	const nested = 1 + 2;

	// Multiple unary operators
	const multiNeg = -a + -b * -c;
	const multiNot = (!a && !b) || !c;
</script>
