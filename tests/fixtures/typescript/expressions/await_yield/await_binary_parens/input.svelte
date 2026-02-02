<script lang="ts">
	// await in binary expressions needs parens for clarity (Prettier style)

	// RHS of nullish coalescing
	const a = b ?? (await c());

	// LHS of arithmetic
	const d = (await e()) + f;

	// RHS of logical OR
	const g = h || (await i());

	// RHS of logical AND
	const j = k && (await l());

	// Both sides
	const m = (await n()) + (await o());

	// In ternary branches - no parens needed
	const p = q ? await r() : await s();

	// yield in binary also needs parens
	function* gen() {
		const t = u || (yield v);
	}
</script>
