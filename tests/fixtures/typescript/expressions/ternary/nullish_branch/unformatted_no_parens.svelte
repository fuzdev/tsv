<script>
	// In consequent
	const a = condition ? x ?? y : z;
	// In alternate
	const b = condition ? z : x ?? y;
	// In both branches
	const c = condition ? x ?? y : a ?? b;
	// In condition
	const d = x ?? y ? a : b;
	// Chained nullish
	const e = condition ? x ?? y ?? z : w;
	// Nested ternary with nullish
	const f = condition ? (inner ? x ?? y : z) : w;
</script>
