<script lang="ts">
	// Catch union type at boundaries

	// 100 chars exactly - stays inline (at boundary)
	try {
	} catch (e: A | B | C | D | E | F | G | LongGenericTypeNameForTestingLineLengthAtExactBoundary) {}

	// 101 chars - union type wraps (exceeds print_width)
	try {
	} catch (e:
		A | B | C | D | E | F | G | LongGenericTypeNameForTestingLineLengthAtExactBoundaryX) {}

	// Catch destructured parameter at boundaries (single-line authored, so width decides)

	// 100 chars exactly - stays inline (at boundary)
	try {
	} catch ({ a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, s, t, u, v, w, x, y, zzz }: T) {}

	// 101 chars - destructured pattern wraps (exceeds print_width)
	try {
	} catch ({
		a,
		b,
		c,
		d,
		e,
		f,
		g,
		h,
		i,
		j,
		k,
		l,
		m,
		n,
		o,
		p,
		q,
		r,
		s,
		t,
		u,
		v,
		w,
		x,
		y,
		zzzz
	}: T) {}
</script>
