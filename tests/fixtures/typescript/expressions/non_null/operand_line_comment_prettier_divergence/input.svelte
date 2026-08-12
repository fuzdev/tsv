<script lang="ts">
	// A line comment before the `!` keeps the operand's parens and its own line.
	const a = (
		x + y // c
	)!;

	// A ternary operand, whose parens are likewise required.
	const b = (
		p ? q : r // c
	)!;

	// The parens are redundant here, and retained for the comment's sake.
	const c = (
		aaa // c
	)!;
</script>
