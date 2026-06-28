<script lang="ts">
	// leading comment already before the operand — both formatters keep it there
	const a = /* b */ (x + y)!;
	const b = /* b */ (p ? q : r)!;
</script>
