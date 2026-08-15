<script lang="ts">
	// A block run glued to `=>` that the author broke after keeps its break when the
	// body breaks: the run takes its own line under `=>`, the body below it.
	const x1 = () => /* c1 */
	fn1({
		b: 1,
		c: 2
	});

	// Width alone materializes the same break: the run holds its own line and the
	// ternary re-fits below it, without its inline parens.
	const x2 = (a) => /* c2 */
	(a ? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb : ccccccccccccccccccccccccccccccccccccccccccccc);

	// A multiline template body breaks the same way.
	const x3 = () => /* c3 */
	`ddd
	eee`;
</script>
