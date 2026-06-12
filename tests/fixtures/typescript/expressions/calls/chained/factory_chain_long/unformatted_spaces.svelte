<script>
	// Factory names (capital letter) merge chain, break at args
	// 100 chars - stays inline (61 as)
	const a = Object.keys().map(
		inner(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)
	);

	// 101 chars - breaks after .map( (62 as)
	const b = Object.keys(

	).map(

		inner(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa),

	);

	// Non-factory names (lowercase) break the chain itself
	// 101 chars - breaks after identifier (62 as)
	const c = object
		.keys(

		)
		.map(
			inner(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa

		));

	// Inner boundary tests: when inner call line hits 100/101 chars
	// Inner line 100 chars - inner call stays inline
	const d = Object.keys()
	.map(
		
	inner(
			a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, s, t, u, v, w, x, y, z, aa, bb, cc),
	);

	// Inner line 101 chars - inner call args break
	const e = Object.keys()
	.map(

		inner(
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
			z,
			aa,
			bb,
			ccc,
		),

	);
</script>
