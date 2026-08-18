<script lang="ts">
	async function fn1() {
		// A `//` in the `new`→callee gap forces the break, so the callee — and the whole
		// tail after it — continues one level down.
		new // c1
		Foo();

		new // c2
		Foo(a, b);

		// The `await`→operand gap is the same gap one keyword over, and takes the same
		// continuation.
		await // c3
		fn2();

		// An operand whose parens are REQUIRED keeps the run OUTSIDE the pair: the gap
		// belongs to the keyword, not to the parens the printer emits.
		await // c4
		(a + b);

		// A run keeps one comment per line, and the whole run rides the continuation.
		new // c5
		// c6
		Foo();

		// An own-line multiline block hangs for the same reason and takes the same indent.
		new /* c7
		c7b */
		Foo();

		// In value position the continuation sits one level under the statement.
		const c =
			await // c8
			fn2();

		// Control: a single-line block forces nothing, so it collapses inline and there is
		// no continuation to indent.
		new /* c9 */ Foo();
	}
</script>
