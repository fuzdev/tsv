<script lang="ts">
	// binary operand — parens required (non-null binds tighter than `+`)
	const a = (x + y /* c */)!;

	// ternary operand
	const b = (p ? q : r /* c */)!;

	// a long operand hangs inside the parens, the block beside its last line and counted in
	// its width
	const c = (
		aaaaaaaaaaaaaaaaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb : cccccccccccccccccc /* c */
	)!;
	const d = (
		aaaaaaaaaaaaaaaaaaaaaaaa
			? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			: ccccccccccccccccccc /* c */
	)!;
</script>
