<script lang="ts">
	// Short - fits inline
	const z = fn<{ a: number; b: string }>;

	// Instantiation expression (no call parens): object type-arg breaks block-style on width
	const x = fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn<{ a: number | string; b: number | string | null }>;

	// Tagged template: object type-arg breaks block-style on width
	const y = taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa<{ a: number | string; b: number | string | null }>`abc`;
</script>
