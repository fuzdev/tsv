<script lang="ts">
	// Short identifier chain as call argument - z should NOT merge with .string()
	// because the chain is not an expression statement (isExpressionStatement=false)
	fn(
		z
			.string()
			.meta({aliases: ['v']})
			.optional(),
	);

	// Deeper nesting - chain inside nested call arguments
	fn1(
		fn2(
			z
				.string()
				.meta({aliases: ['v']})
				.optional(),
		),
	);

	// Contrast: same chain as expression statement - z SHOULD merge with .string()
	// (isShort heuristic applies to expression statements only)
	z.string()
		.meta({aliases: ['v']})
		.optional();
</script>
