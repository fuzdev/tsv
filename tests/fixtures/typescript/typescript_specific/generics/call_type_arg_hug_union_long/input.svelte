<script lang="ts">
	// Boundary: exactly 100 - single hugged union type-arg stays fully inline
	const w100 = someUnionArgPredicateCheckerFn<{ aKey: number } | null>(firstPredicateArgumentValue);

	// Boundary: 101 - object breaks block-style, `<{` hugs and `} | null>` stays together
	const w101 = someUnionArgPredicateCheckerFn<{ aKey: number } | null>(
		firstPredicateArgumentValues
	);

	// Null-first hugged union - same hugging
	const nullFirst = someGenericPredicateCheckerForUnionTypeArgHere<null | {
		aKey: number;
		bKey: string;
	}>(firstPredicate);

	// Union with void member - also hugs
	const voidMember = someGenericPredicateCheckerForUnionTypeArgHere<{
		aKey: number;
		bKey: string;
	} | void>(firstPredicate);
</script>
