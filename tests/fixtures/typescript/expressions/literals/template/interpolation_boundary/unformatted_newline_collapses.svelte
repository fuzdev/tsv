<script lang="ts">
	// Interpolation fits at printWidth (100 chars) — inline
	const a = `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ${
		expr
	} yyyyyyyy`;

	// Compact source over printWidth (101 chars) — stays inline (atomized)
	const b = `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ${expr} yyyyyyyy`;

	// Source spans lines over printWidth — breaks to respect printWidth
	const c = `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ${
		expr
	} yyyyyyyy`;
</script>
