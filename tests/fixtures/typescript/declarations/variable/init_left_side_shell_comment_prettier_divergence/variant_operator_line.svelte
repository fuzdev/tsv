<script lang="ts">
	// An own-line comment inside the grouping parens of the initializer's LEFTMOST node — a
	// conditional's test, a member's object, a callee. The initializer's printer hoists the
	// run ahead of its whole doc, and the declarator hangs the value under `=` so the run and
	// the value share the value's indent — the shape a binary initializer already takes.
	const a = // c
		(x as any) ? y : z;
	const b = // c
		(x as any).p;
	const c =
		// c
		x();
	const d =
		// c
		(x as any) + y;
</script>
