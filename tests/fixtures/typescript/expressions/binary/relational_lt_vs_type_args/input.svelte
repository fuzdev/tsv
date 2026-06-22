<script lang="ts">
	// `<` before a literal is a comparison operator, not type arguments
	const a1 = x < 'b';
	const a2 = x < 'b' && y < 'c';
	const a3 = x < `b`;
	const a4 = x < true;
	const a5 = x < null;
	const a6 = x < { a: 1 };
	const a7 = x < [1];

	// a closing `>` followed by an identifier is a chained comparison, not type args;
	// this holds across string, numeric, and bare-identifier operands
	const a8 = x < 'b' > c;
	const a9 = x < 1 > c;
	const a10 = a < b > c;
	const a11 = a < b < c;

	// a parenthesized ternary operand is a comparison, not a `(b?: T) => ...` function type
	const a12 = x < (b ? c : d);

	// a `>` inside an array literal belongs to the array, not the relational chain
	const a13 = x < [a > b];

	// any expression-starting token after the closing `>` makes it a comparison
	// chain too — literal, unary, array, and object operands behave like the
	// identifier and typeof operands
	const a14 = x < y > 1;
	const a15 = x < y > 'b';
	const a16 = x < y > !c;
	const a17 = x < y > ~c;
	const a18 = x < y > -1;
	const a19 = x < y > +1;
	const a20 = x < y > [0];
	const a21 = x < y > { a: 1 };
	const a22 = x < y > typeof c;

	// genuine instantiation with a literal/keyword/object/tuple/numeric type arg, or a
	// function type with an optional param, still parses as type arguments
	const b1 = fn<'b'>();
	const b2 = fn<true>();
	const b3 = fn<string>();
	const b4 = fn<{ a: number }>();
	const b5 = fn<[A, B]>();
	const b6 = fn<1>();
	const b7 = fn<(b?: T) => U>();

	// a generic nested inside a tuple or object-type arg still parses as type arguments
	const b8 = fn<[T<A>]>();
	const b9 = fn<[A, T<B>]>();
	const b10 = fn<{ a: T<B> }>();
	const b11 = fn<[T<A, B>]>();
	const b12 = fn<[T<A>[]]>();
	const b13 = fn<[T<A>], B>();
	const b14 = fn<[[T<A>]]>();

	// a template literal after the closing `>` is a tagged template on the
	// instantiation, not a comparison
	const b15 = fn<A>`b`;
</script>
