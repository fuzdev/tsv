<script lang="ts">
	// A multi-line block comment leading an expression STATEMENT must not BREAK the
	// expression's own group: prettier prints a leading comment outside that group, so an
	// expression that fits stays flat below the comment. Same rule as at a value gap
	// (`operator_value_indentable_block_comment_flat`), one gap family over.

	// binary chain
	/**
	 * c
	 */ b1 + c1 + d1;

	// ternary
	/**
	 * c
	 */ b2 ? c2 : d2;

	// member chain
	/**
	 * c
	 */ b3.c3().d3().e3();

	// a preserved (non-indentable) multi-line block asks the same question
	/* c
d */ b4 + c4 + d4;

	// inside a block body, where the statement list is the same seam
	function fn1() {
		/**
		 * c
		 */ b5 + c5 + d5;
	}

	// null control: a single-line block forces no break
	/* c */ b6 + c6 + d6;

	// null control: no comment
	b7 + c7 + d7;
</script>
