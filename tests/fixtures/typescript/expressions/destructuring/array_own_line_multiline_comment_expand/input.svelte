<script>
	// Own-line multiline block comment after the last element
	const [
		a,
		b
		/* c1
c2 */
	] = arr;

	// Own-line multiline block comment between elements
	const [
		a1,
		/* c1
c2 */
		b1
	] = arr;

	// Own-line multiline block comment before the first element
	const [
		/* c1
c2 */
		a2,
		b2
	] = arr;
</script>
