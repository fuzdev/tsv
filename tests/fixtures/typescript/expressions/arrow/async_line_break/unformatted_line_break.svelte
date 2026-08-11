<script lang="ts">
	// `AsyncArrowFunction : async [no LineTerminator here] ArrowFormalParameters
	// => AsyncConciseBody` (ecma262). A line terminator between `async` and the
	// parameters is a syntax error; the `input_invalid_*` files cover the
	// rejections. These valid baselines keep the parameters on the `async` line.
	const f = async () => {}
	const h = async a => a + 1

	// a line break before a SINGLE parameter is not an error — ASI splits it into
	// an `async` identifier reference and a separate arrow expression statement
	const j = async
	a => a + 1
</script>
