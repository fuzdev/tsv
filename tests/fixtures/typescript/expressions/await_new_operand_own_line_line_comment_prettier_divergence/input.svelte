<script>
	async function fn1() {
		// An own-line `//` in the `new`→callee gap keeps the line the author gave it; the
		// callee — and the whole tail after it — hangs one level down below the run.
		new
			// c1
			Foo();

		// The `await`→operand gap is the same gap one keyword over, and takes the same
		// answer.
		await
			// c2
			fn2();

		// An operand whose parens are REQUIRED keeps the run OUTSIDE the pair: the gap
		// belongs to the keyword, not to the parens the printer emits.
		await
			// c3
			(a + b);

		// A run keeps one comment per line, in order.
		new
			// c4
			// c5
			Foo();

		// A block ahead of the line comment keeps its own line too.
		new
			/* c6 */
			// c7
			Foo();

		// An own-line multiline block hangs for the same reason and keeps its own line.
		new
			/* c8
		c8b */
			Foo();

		// In value position the run sits one level under the statement.
		const c = await
			// c9
			fn2();

		// Control: a comment the author put ON the keyword's line trails it, as before.
		new // c10
			Foo();
	}
</script>
