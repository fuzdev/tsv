<script lang="ts">
	// A multi-line block comment leading an arrow BODY must not BREAK the body's own
	// group: prettier prints a leading comment outside that group, so a body that fits
	// stays flat below the comment. The `=>`→body gap is a value gap like the others
	// (`operator_value_indentable_block_comment_flat`); only the operator differs.

	// binary chain
	const a1 = () =>
		/**
		 * c
		 */ b1 + c1 + d1;

	// ternary
	const a2 = () =>
		/**
		 * c
		 */ b2 ? c2 : d2;

	// member chain
	const a3 = () =>
		/**
		 * c
		 */ b3.c3().d3().e3();

	// a preserved (non-indentable) multi-line block asks the same question
	const a4 = () =>
		/* c
d */ b4 + c4 + d4;

	// the same body as a call argument
	fn1(
		() =>
			/* c
d */ b5 + c5 + d5
	);

	// null control: a single-line block forces no break
	const a6 = () => /* c */ b6 + c6 + d6;

	// null control: no comment
	const a7 = () => b7 + c7 + d7;
</script>
