<script lang="ts">
	// a block comment glued to a grouping `(` binds to the parenthesized value, so the pair is kept
	// when its interior opens with a `//`: stripped, the `//` would land between the two
	async function fn() {
		await /* c1 */ (
			// d1
			a
		);
		await /* c27 */ (

			// d27
			(

				// e27
				b
			)
		);
		await /* c28 */ (
			// d28

			b
		);
		await /* c29 */ (
			/* e29 */

			// d29
			b
		);
		await /* c30 */ (
			/* e30 */
			// d30
			b
		);
		await /* a31 */ /* c31 */ (
			// d31
			b
		);
		await /* c32
			d32 */ (
			// e32
			b
		);
	}
	new /* c2 */ (
		// d2
		A
	)();
	switch (x) {
		case /* c3 */ (
			// d3
			y
		):
			break;
	}
	function fn2() {
		return /* c4 */ (
			// d4
			a
		);
	}
	function fn3() {
		throw /* c5 */ (
			// d5
			a
		);
	}
	function* fn4() {
		yield /* c6 */ (
			// d6
			a
		);
	}
	void (
		/* c7 */ (
			// d7
			a
		)
	);
	const v1 = /* c8 */ (
		// d8
		a
	);
	v2 = /* c9 */ (
		// d9
		a
	);
	class A1 {
		p = /* c10 */ (
			// d10
			a
		);
	}
	enum E {
		A = /* c11 */ (
			// d11
			a
		)
	}
	const o = {
		a: /* c12 */ (
			// d12
			a
		)
	};
	const f1 = () =>
		/* c13 */ (
			// d13
			a
		);
	fn(
		/* c14 */ (
			// d14
			a
		)
	);
	const a1 = [
		/* c15 */ (
			// d15
			a
		)
	];
	const s = `${
		/* c16 */ (
			// d16
			a
		)
	}`;
	function fn5(
		p = /* c17 */ (
			// d17
			a
		)
	) {}
	const {
		p = /* c18 */ (
			// d18
			a
		)
	} = o;
	const v3 =
		b +
		/* c19 */ (
			// d19
			a
		);
	const v4 = cond
		? /* c20 */ (
				// d20
				a
			)
		: b;
	/* c21 */ (
		// d21
		a
	);
	for (const p of /* c22 */ (
		// d22
		a
	));
	fn(
		.../* c23 */ (
			// d23
			a
		)
	);
	const v5 = /* c24 */ (
		// d24
		a
	).b();
	const v6 =
		/* c25 */ (
			// d25
			a
		) + b;
	const v7 = /* @__PURE__ */ (
		// side-effect free
		fn()
	);
	// the same cells with the `//` glued to the `(`, where it keeps its line: the
	// opening-delimiter rule
	{
		async function fn() {
			await /* g1 */ ( // h1
				a
			);
			await /* g27 */ ( // h27

				// i27
				b
			);
			await /* g28 */ ( // h28

				b
			);
			await /* f31 */ /* g31 */ ( // h31
				b
			);
			await /* g32
				h32 */ ( // i32
				b
			);
		}
		new /* g2 */ ( // h2
			A
		)();
		switch (x) {
			case /* g3 */ ( // h3
				y
			):
				break;
		}
		function fn2() {
			return /* g4 */ ( // h4
				a
			);
		}
		function fn3() {
			throw /* g5 */ ( // h5
				a
			);
		}
		function* fn4() {
			yield /* g6 */ ( // h6
				a
			);
		}
		void (
			/* g7 */ ( // h7
				a
			)
		);
		const v1 = /* g8 */ ( // h8
			a
		);
		v2 = /* g9 */ ( // h9
			a
		);
		class A1 {
			p = /* g10 */ ( // h10
				a
			);
		}
		enum E {
			A = /* g11 */ ( // h11
				a
			)
		}
		const o = {
			a: /* g12 */ ( // h12
				a
			)
		};
		const f1 = () =>
			/* g13 */ ( // h13
				a
			);
		fn(
			/* g14 */ ( // h14
				a
			)
		);
		const a1 = [
			/* g15 */ ( // h15
				a
			)
		];
		const s = `${
			/* g16 */ ( // h16
				a
			)
		}`;
		function fn5(
			p = /* g17 */ ( // h17
				a
			)
		) {}
		const {
			p = /* g18 */ ( // h18
				a
			)
		} = o;
		const v3 =
			b +
			/* g19 */ ( // h19
				a
			);
		const v4 = cond
			? /* g20 */ ( // h20
					a
				)
			: b;
		/* g21 */ ( // h21
			a
		);
		for (const p of /* g22 */ ( // h22
			a
		));
		fn(
			.../* g23 */ ( // h23
				a
			)
		);
		const v5 = /* g24 */ ( // h24
			a
		).b();
		const v6 =
			/* g25 */ ( // h25
				a
			) + b;
		const v7 = /* @__PURE__ */ ( // side-effect free
			fn()
		);
	}
</script>
