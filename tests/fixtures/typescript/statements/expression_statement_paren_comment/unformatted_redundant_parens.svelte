<script>
	// Each statement below is authored with a redundant paren pair around its
	// expression (see the variant); the paren is dropped and the comment inside
	// it leads the statement.
	(
	// c1
	a);

	(
	/* c2 */
	b);

	(
	// c3
	c, d);

	((
	// c4
	e));

	// A block glued to the expression leads it inline and stays on its line
	(/* c5 */ f);

	// A run the author glued onto one line stays glued
	(
	/* c6 */ /* c7 */
	g);

	// A blank line between two comments is preserved
	(
	// c8

	// c9
	h);
</script>
