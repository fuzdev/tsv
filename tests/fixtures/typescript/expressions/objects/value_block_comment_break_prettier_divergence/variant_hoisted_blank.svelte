<script lang="ts">
	// A block comment glued to `:` keeps its position; the value reflows onto the
	// `:` line. Prettier instead hoists the comment above the property.
	const o = {
		/* c */

		k: 1
	};
</script>
