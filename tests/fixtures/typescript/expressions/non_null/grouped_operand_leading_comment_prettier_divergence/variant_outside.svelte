<script lang="ts">
	// leading comment already before the operand
	const a = /* b */ (x + y)!;
	const b = /* b */ (p ? q : r)!;
</script>
