<script lang="ts">
	// A multi-line block comment leading a call ARGUMENT must not BREAK the argument's
	// own group: prettier prints a leading comment outside that group, so an argument
	// that fits stays flat below the comment. Same rule as at a value gap
	// (`operator_value_indentable_block_comment_flat`), one gap family over.

	// binary chain
	fn1(
		/**
		 * c
		 */ b1 + c1 + d1
	);

	// ternary
	fn2(
		/**
		 * c
		 */ b2 ? c2 : d2
	);

	// member chain
	fn3(
		/**
		 * c
		 */ b3.c3().d3().e3()
	);

	// a preserved (non-indentable) multi-line block asks the same question
	fn4(
		/* c
d */ b4 + c4 + d4
	);

	// a later argument reads the same
	fn5(
		e5,
		/**
		 * c
		 */ b5 + c5 + d5
	);

	// `new`, which prints its arguments through the same ladder
	const a1 = new Fn6(
		/**
		 * c
		 */ b6 + c6 + d6
	);

	// null control: a single-line block forces no break
	fn7(/* c */ b7 + c7 + d7);

	// null control: no comment
	fn8(b8 + c8 + d8);

	export { a1 };
</script>
