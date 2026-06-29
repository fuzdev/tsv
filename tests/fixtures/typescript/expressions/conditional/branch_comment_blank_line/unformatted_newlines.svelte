<script lang="ts">
	// block comment + blank line in the consequent gap forces the ternary to break
	const t1 = a
		? /* c */



			b
		: c;

	// block comment + blank in the alternate gap
	const t2 = a
		? b
		: /* c */



			c;

	// line comment + blank: blank preserved in the broken layout
	const t3 = a
		? // c



			b
		: c;

	// block comment with no blank stays inline (both formatters)
	const t4 = a ? /* c */ b : c;

	// two comments with a blank between: the blank is preserved and the second
	// comment (now on its own line) drops the consequent below it
	const t5 = a
		? /* a */



			/* b */
			b
		: c;
</script>
