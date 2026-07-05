<script lang="ts">
	// Wrap SHAPE when a newline'd interpolation overflows, by expression type:
	// qualifying types break at the ${ / } boundaries; non-qualifying types hug
	// ${ / } and break internally.
	const qualifying_ternary = `${
		conditionAAAAAAAAAAAAAAAAAAAAA ? consequentBBBBBBBBBBBBBBBBBB : alternateCCCCCCCCCCCCCCCCCC
	}`;

	const nonqualifying_call = `${functionName(
		argumentOneeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee,
		argumentTwooooooooooooooooooooooooooooooooo
	)}`;

	const nonqualifying_array = `${[
		elementOneeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee,
		elementTwooooooooooooooooooooooooooooooooooo
	]}`;
</script>
