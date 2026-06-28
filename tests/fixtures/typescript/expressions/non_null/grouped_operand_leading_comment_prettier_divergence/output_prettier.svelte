<script lang="ts">
	// leading comment inside the required parens
	const a = /* b */ (x + y)!;

	// ternary operand
	const b = /* b */ (p ? q : r)!;
</script>
