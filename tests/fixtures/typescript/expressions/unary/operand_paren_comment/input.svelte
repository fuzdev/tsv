<script>
	// block comment after a simple operand
	!(x /* c */);
	typeof (x /* c */);
	void (x /* c */);
	~(x /* c */);

	// block comment before operand
	!(/* c */ x);
	typeof (/* c */ x);

	// block comment after a binary/logical operand (redundant parens stripped)
	!(x + y /* c */);
	!(x || y /* c */);

	// block comment before a binary operand
	!(/* c */ x + y);

	// assignment and ternary operands keep their parens
	!((x = y) /* c */);
	!((a ? b : c) /* c */);

	// multiline block comment after a binary operand
	!(
		x + y
		/* c */
	);

	// line comment after a binary operand
	!(
		x + y // c
	);

	// multiline block comment before operand
	!(
		/* c */
		x
	);
</script>
