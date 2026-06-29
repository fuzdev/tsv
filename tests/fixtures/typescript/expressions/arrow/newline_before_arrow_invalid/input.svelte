<script lang="ts">
	// `ArrowFunction : ArrowParameters [no LineTerminator here] => ConciseBody`
	// (ecma262). A line terminator between the parameters (or return type) and
	// `=>` is a syntax error; the `input_invalid_*` files cover the rejections.
	// These valid baselines keep the `=>` on the signature line.
	const f = (a) => a + 1;
	const h = (a): number => a + 1;
	const j = async (a) => a + 1;

	// a same-line block comment carries no line terminator, so it stays valid
	const k = (a) /* c */ => a + 1;
</script>
