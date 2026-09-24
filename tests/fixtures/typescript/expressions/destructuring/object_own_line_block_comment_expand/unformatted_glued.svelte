<script>
	// own-line block comment between properties forces expansion
	const {
		a,
		/* c */
		b
	} = obj;

	// own-line block comment before first property forces expansion
	const {
		/* c */
		a1,
		b1
	} = obj;

	// own-line block comment after last property forces expansion
	const {
		a2,
		b2
		/* c */
	} = obj;

	// same-line block comment does NOT force expansion (short pattern)
	const {a3,
/* c */b3} = obj;

	// an own-line MULTI-LINE block comment after the last property keeps its own line too
	const {
		a4,
		b4
		/* c1
c2 */
	} = obj;

	({
		a5,
		b5
		/* c1
c2 */
	} = obj);
</script>
