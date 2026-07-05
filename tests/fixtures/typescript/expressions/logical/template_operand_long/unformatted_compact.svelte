<script lang="ts">
	// Binary operator with a template operand exceeding printWidth (101 chars).
	// Compact source: the ${...} stays inline (atomized) in both tsv and prettier.
	const x = conditionAAAAAAAAAAAAAAAA && `${aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}`;
</script>
