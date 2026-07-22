<script>
	// A single-line block comment after `await`/`new` collapses inline — whether
	// authored glued, trailing the keyword, or on its own line. The own-line
	// authoring (unformatted_ours_own_line) collapses to this in one pass.
	async function fn() {
		const a = await
		/* a */
		foo;

		const b = new
		/* b */
		Foo();
	}
</script>
