<script>
	// Prettier's `isPoorlyBreakableMemberOrCallChain` descends a member's OBJECT only and never
	// reads a computed member's index, so an index that forces a break leaves the chain poorly
	// breakable: the value breaks after the operator and the index expands inside it

	// a multiline template call in the computed member's index, at every assignment operator
	z =
		w.x[
			o(`
a
`)
		];
	z +=
		w.x[
			o(`
a
`)
		];
	z ??=
		w.x[
			o(`
a
`)
		];
	a.b =
		w.x[
			o(`
a
`)
		];

	// a class property, and an object property: a short key keeps the value on its line, a longer
	// key breaks after the colon
	class C {
		p =
			w.x[
				o(`
a
`)
			];
	}
	q = {
		k: w.x[
			o(`
a
`)
		],
		kkkkk:
			w.x[
				o(`
a
`)
			]
	};

	// a declarator takes the same layout
	const y =
		w.x[
			o(`
a
`)
		];

	// a bare or tagged multiline template as the index, at any depth
	z =
		w[
			`
a
`
		];
	z =
		w.x[
			t`
a
`
		];
	z =
		w.x.y[
			o(`
a
`)
		];

	// the operand of a wrapper, or a chain rooted at `this` or reached through `?.`
	z =
		await w.x[
			o(`
a
`)
		];
	z =
		!w[
			`
a
`
		];
	z =
		this.x[
			o(`
a
`)
		];
	z =
		w.x?.[
			o(`
a
`)
		];

	// the chain continuing past the bracket
	z =
		w.x[
			o(`
a
`)
		].y;
	z =
		w.x[
			o(`
a
`)
		]();
	z =
		w[
			`
a
`
		].f(x);

	// other indexes that force a break: a block body, an authored-expanded object
	z =
		w[
			() => {
				f();
			}
		];
	z =
		w[
			{
				a: 1
			}
		];

	// control: the template as a call argument instead keeps the chain on the operator line
	z = w.x(
		o(`
a
`)
	);
</script>
