<script lang="ts">
	// A relational chain keeps a paren pair around its `<` operand whenever the region
	// between the `<` and the `>` would parse as a type-argument list — and the token past
	// the `>` is not the axis. Every follower here is an expression head, which is exactly
	// what makes the input a comparison chain; bare, the first break the printer takes at
	// the `>` re-lexes every one of them for tsv's own parse, which commits a type-argument
	// list past a line terminator ahead of any expression (tsc keeps the chain at `+` / `-`).

	// A string, a numeric and a bare-identifier `<` operand ask the same question.
	const a1 = (x < 'b') > c;
	const a2 = (x < 1) > c;
	const a3 = (a < b) > c;

	// So does every follower shape: a literal, a prefix operator, an array, an object and
	// a `typeof` operand.
	const a4 = (x < y) > 1;
	const a5 = (x < y) > 'b';
	const a6 = (x < y) > !c;
	const a7 = (x < y) > ~c;
	const a8 = (x < y) > -1;
	const a9 = (x < y) > +1;
	const a10 = (x < y) > [0];
	const a11 = (x < y) > { a: 1 };
	const a12 = (x < y) > typeof c;

	// An indexed operand is as much a type as a reference one, so it takes the pair too.
	const a13 = (a < B[c]) > d;
</script>
