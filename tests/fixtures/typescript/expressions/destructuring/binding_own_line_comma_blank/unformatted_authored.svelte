<script>
	// a blank line counts only when it starts on the line the binding ends on, so a comma the
	// author pushed onto its own line takes the following blank out of the gap
	const {
		// c1
		a1
		,

		b1
	} = o;

	// authored the other way round, the blank does start on that line and is preserved
	const {
		// c2
		a2

		,
		b2
	} = o;

	// an array pattern takes the other rule - it measures from the comma, so a blank after an
	// own-line comma is preserved there
	const [
		// c3
		a3
		,

		b3
	] = o;
</script>
