<script>
	// Outer boundary tests: this.fn().call(inner(...))
	// Tests whether chain stays together or breaks after `this`

	// 100 chars - stays inline (64 a's)
	const a = this.fn(
		
	).call(
		inner(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		)
		);

	// 101 chars - breaks after .call( (65 a's)
	const b = this.fn().call(
		
	inner(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa),
	);

	// Inner boundary tests: when inner call line hits 100/101 chars
	// Tests whether inner call args stay inline or break

	// Inner line 100 chars - inner call stays inline
	const c = this.fn().call(
		inner(
			a, b, c, d, e,
			 f, g, h, i, j, k, l
			 , m, n, o, p, q, r, s, t, u, v, w, x, y, z, aa, bb, cc),
	);

	// Inner line 101 chars - inner call args break
	const d = this.fn(

	)
	.call(
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
