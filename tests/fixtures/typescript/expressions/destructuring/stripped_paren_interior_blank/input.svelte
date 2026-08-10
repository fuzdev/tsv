<script>
	// a blank line INSIDE a stripped paren shell goes with the shell; only a blank the
	// author wrote between two items survives
	const {
		a1 = 1,
		b1,

		c1
	} = x;

	const [
		a2 = 1,
		b2,

		c2
		// a comment expands the array pattern, which a blank line alone does not
	] = x;

	function fn(
		a3 = 1,
		b3,

		c3
	) {}
</script>
