<script>
	// block comment after operand (paren stripped, comment lost)
	!(x /* c */);
	typeof (x /* c */);
	void (x /* c */);
	~(x /* c */);

	// block comment before operand
	!(/* c */ x);
	typeof (/* c */ x);

	// block comment after binary operand (paren kept, comment lost)
	!((x + y) /* c */);
	!((x || y) /* c */);
	!((x = y) /* c */);

	// multiline block comment after operand
	!(
		x
		/* c */
	);

	// line comment after operand
	!(
		x // c
	);
	typeof (
		x // c
	);

	// multiline block comment before operand
	!(
		/* c */
		x
	);
</script>
