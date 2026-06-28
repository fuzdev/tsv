<script lang="ts">
	// line comment in a non-null grouped operand — kept inside, forcing multiline
	const a = (
		x + y // c
	)!.foo;

	// ternary operand
	const b = (
		p ? q : r // c
	)!.foo;
</script>
