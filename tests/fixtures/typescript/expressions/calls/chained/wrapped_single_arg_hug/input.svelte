<script lang="ts">
	// In a broken member chain, a single TS-wrapped object arg hugs the call paren
	// (classify_chain_arg looks through `satisfies`/`<T>` to the object). The object
	// expands internally while the wrapper suffix trails the closing brace.

	// `satisfies` wrapping the single arg
	const a = somethingWithAReasonablyLongNameForChaining
		.firstMethodCall()
		.secondMethodCall()
		.thirdMethodCall({
			aaaa: 1,
			bbbb: 2
		} satisfies Ttttttt);

	// angle-bracket assertion wrapping the single arg
	const b = somethingWithAReasonablyLongNameForChaining
		.firstMethodCall()
		.secondMethodCall()
		.thirdMethodCall(<Ttttttt>{
			aaaa: 1,
			bbbb: 2
		});
</script>
