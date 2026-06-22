<script lang="ts">
	// Case 1: Outer call boundary
	// 100 chars - outer fits on one line
	outer(a, ns.inner(cond ? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb : c));

	// 101 chars - outer breaks, inner stays inline
	outer(
		a,
		ns.inner(cond ? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb : c)
	);

	// Case 2: Inner call boundary (outer already broken)
	// 100 chars - inner call stays inline with ternary
	outer(
		a,
		ns.inner(cond ? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb : c)
	);

	// 101 chars - inner call breaks, ternary on one line
	outer(
		a,
		ns.inner(
			cond ? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb : c
		)
	);

	// Case 3: Ternary boundary (inner call already broken)
	// 100 chars - ternary stays on one line
	outer(
		a,
		ns.inner(
			cond ? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb : c
		)
	);

	// 101 chars - ternary must break
	outer(
		a,
		ns.inner(
			cond
				? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
				: c
		)
	);
</script>
