<script lang="ts">
	// line comment in a non-null grouped operand forces the multiline paren layout
	const a = (x +
		y)! // c
	.foo;

	// ternary operand
	const b = (
		p ? q : r
	)! // c
	.foo;
</script>
