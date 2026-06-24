<script lang="ts">
	// Inline object type-arg fits at exactly 100 - stays inline
	const a = fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn<{ a: number | string; b: number | string | null }>();

	// One char over (101) - the object type-arg breaks block-style, not the inner union
	const b = fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn<{
		a: number | string;
		b: number | string | null;
	}>();

	// Non-union members - same block-style break (the object expands, not anything inside it)
	const c = fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn<{
		a: number;
		b: string;
		c: boolean;
	}>();
</script>
