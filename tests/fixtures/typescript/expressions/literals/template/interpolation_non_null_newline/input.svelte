<script lang="ts">
	// Non-null assertion in a value interpolation. Prettier's stripChainElementWrappers
	// peels the `!` before the qualifying-member test, so a non-null MEMBER chain
	// qualifies like the bare member (breaks at ${ / }); a non-null CALL does not
	// (stays hugged, breaks internally). tsv mirrors this by peeling TSNonNullExpression.

	// Member chain, source newline -> breaks at ${ / } boundaries, not mid-chain.
	const member = `${
		object.propertyAAAAAAAAAAAAAAAAAAAAA.propertyBBBBBBBBBBBBBBBBBBB.propertyCCCCCCCCCCCCCCCC!
	}`;

	// Member chain, compact source -> stays inline (atomized); the `!` alone never breaks.
	const member_compact = `${object.propertyAAAAAAAAAAAAAAAAAAAAA.propertyBBBBBBBBBBBBBBBBBBB.propertyCCCCCCCCCCCCCCCC!}`;

	// Call, source newline -> non-qualifying (strip -> CallExpression): ${ hugs, args break.
	const call = `${functionNameAAAAAAAAAAAAAAAAAAAAAAAAA(
		argumentOneBBBBBBBBBBBBBBBBBBBBBBBBBB,
		argumentTwoCCCCCCCCCCCCCCCCCCCCCCCCCC
	)!}`;
</script>
