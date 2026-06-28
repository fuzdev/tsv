<script lang="ts">
	// binary operand — parens required (non-null binds tighter than `+`)
	const a = (x + y /* c */)!;

	// ternary operand
	const b = (p ? q : r /* c */)!;
</script>
