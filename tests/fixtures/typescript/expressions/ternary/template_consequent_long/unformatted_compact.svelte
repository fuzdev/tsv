<script lang="ts">
	// Ternary with a template consequent exceeding printWidth (101 chars).
	// Compact source: the ${...} stays inline (atomized) in both tsv and prettier.
	const x = conditionAAAAAAAAAAAAAAAA ? `${aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}` : fallback;
</script>
