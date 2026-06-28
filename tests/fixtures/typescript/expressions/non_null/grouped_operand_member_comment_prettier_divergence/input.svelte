<script lang="ts">
	// binary operand, non-null then member access — parens required
	const a = (x + y /* c */)!.foo;

	// ternary operand
	const b = (p ? q : r /* c */)!.foo;

	// longer member chain after the assertion
	const c = (x + y /* c */)!.foo.bar;

	// call after the assertion
	const d = (x + y /* c */)!();
</script>
