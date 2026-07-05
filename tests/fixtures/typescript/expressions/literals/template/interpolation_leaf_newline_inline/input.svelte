<script lang="ts">
	// Non-qualifying leaf types (string Literal, UnaryExpression) are NOT in the
	// ${/} softline-wrap list, so even with a source newline they stay inline
	// (atomized) past printWidth - like prettier. Contrast the qualifying chain /
	// binary / ternary in interpolation_newline_positions, which break instead.
	const str = `${'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'}`;
	const unary = `${typeof aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}`;
</script>
