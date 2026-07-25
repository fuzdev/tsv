<script lang="ts">
	// `>=` after a `<` operand is a relational chain — the `>` belongs to the `>=`
	// operator, not to a type-argument close followed by `=`. The bare-identifier and
	// indexed-access operands agree.
	const a1 = a < b >= d;
	const a2 = a < B[c] >= d;
</script>
