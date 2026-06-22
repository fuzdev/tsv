<script lang="ts">
	// Both args are TSAsExpression - expand-last disabled (same outer type)
	const a = fn(
		x as unknown as A,
		{
			a: (error: unknown) => Promise.resolve(transform(error))
		} as any
	);

	// Both args are TSSatisfiesExpression - expand-last disabled
	const b = fn(
		x satisfies A,
		{
			a: 1,
			b: 2
		} satisfies B
	);
</script>
