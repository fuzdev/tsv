<script lang="ts">
	// In a broken member chain, a `{...}!` / `[...]!` single arg does NOT hug the
	// call paren — the non-null wrapper isn't a cast classify_chain_arg looks
	// through, so the arg soft-wraps onto its own line.

	// object non-null
	const a = somethingWithAReasonablyLongNameForChaining
		.firstMethodCall()
		.secondMethodCall()
		.thirdMethodCall(
			{
				aaaa: 1,
				bbbb: 2
			}!
		);

	// array non-null
	const b = somethingWithAReasonablyLongNameForChaining
		.firstMethodCall()
		.secondMethodCall()
		.thirdMethodCall(
			[elementaaaaaaaa, elementbbbbbbbb, elementcccccccc, elementdddddddd, elementeeee]!
		);
</script>
