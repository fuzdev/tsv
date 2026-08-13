<script lang="ts">
	// A blank the author wrote before the comma is the previous argument's, and the
	// comment that follows leads the next one.
	const a = fn(
		arg1,

		// c1
		arg2
	);

	// The same with a block comment leading the next argument.
	const b = fn(
		arg1,

		/* c2 */
		arg2
	);

	// A comment the author left on the comma's own line.
	const c = fn(
		arg1, // c3

		arg2
	);

	// `new` arguments take the same rule.
	const d = new Fn(
		arg1,

		// c4
		arg2
	);

	// So do a dynamic import's options.
	const e = import(
		'./a',

		// c5
		{ with: { type: 'json' } }
	);
</script>
