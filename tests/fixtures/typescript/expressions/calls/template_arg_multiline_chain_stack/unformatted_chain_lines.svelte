<script lang="ts">
	// A call whose sole argument is a multiline template on the `(` line prints flat, above
	// the member-chain layout, and so does each such call reached as the object of the
	// next link: a stack of them stays glued however many links it has.

	// Two, three and four links below a short head
	v.m(r).o(`
a
`).o(`
b
`);
	v.m(r)
		.o(`
a
`)
		.o(`
b
`).o(`
c
`);
	v.m(r)
		.o(`
a
`)
		.o(`
b
`)
		.o(`
c
`).o(`
d
`);

	// Ending in a member, a non-null or a computed access instead of a call
	v.m(r)
		.o(`
a
`)
		.o(`
b
`).p;
	v.m(r)
		.o(`
a
`)
		.o(`
b
`)
		.o(`
c
`).p;
	v.m(r)
		.o(`
a
`)
		.o(`
b
`)!;
	v.m(r)
		.o(`
a
`)
		.o(`
b
`)[0];

	// A member over a member: every lookup prints after the stack, none on a chain line
	v.m(r)
		.o(`
a
`)
		.o(`
b
`)
		.p.q;

	// A parenthesized optional chain the access seals keeps its parens — under a member, a
	// non-null, a declaration, an assignment target and a `new` callee, and as the head of a
	// further link. An optional access needs no parens, so they go
	(v?.m(r)
		.o(`
a
`)).p;
	(v?.m(r)
		.o(`
a
`))!.p;
	const z = (v?.m(r)
		.o(`
a
`)).p;
	(v?.m(r)
		.o(`
a
`)).p = 1;
	new (v?.m(r)
		.o(`
a
`)).p();
	(v?.o(`
a
`))
		.o(`
b
`).p;
	v?.m(r)
		.o(`
a
`)?.p;

	// Links reached through `?.`, `!` and a computed callee, a tagged template, and a
	// template whose interpolation sits between its lines
	v?.m(r)
		?.o(`
a
`)
		?.o(`
b
`)?.o(`
c
`);
	v.m(r)!
		.o(`
a
`)!
		.o(`
b
`)!.o(`
c
`);
	v.m(r)
		[k](`
a
`)
		[k](`
b
`)[k](`
c
`);
	v.m(r)
		.o(t`
a
`)
		.o(t`
b
`).o(t`
c
`);
	v.m(r)
		.o(`
a ${b}
`)
		.o(`
c ${d}
`).o(`
e ${f}
`);

	// A plain link between template links: the chain below it is laid out on its own
	v.m(r)
		.o(
			`
a
`
		)
		.q(x).o(`
c
`);

	// A test call at the bottom of the stack
	it('a', async () => {
		b();
	})
		.o(`
a
`)
		.o(`
b
`).o(`
c
`);

	// Positions: an assignment, a declaration, `return`, `await`, an arrow body, a nested
	// argument, an object property
	x = v
		.m(r)
		.o(`
a
`)
		.o(`
b
`).o(`
c
`);
	const y = v
		.m(r)
		.o(`
a
`)
		.o(`
b
`).o(`
c
`);
	function f() {
		return v
			.m(r)
			.o(`
a
`)
			.o(`
b
`).o(`
c
`);
	}
	async function g() {
		await v
			.m(r)
			.o(`
a
`)
			.o(`
b
`).o(`
c
`);
	}
	const h = () =>
		v
			.m(r)
			.o(`
a
`)
			.o(`
b
`).o(`
c
`);
	fn(
		v
			.m(r)
			.o(`
a
`)
			.o(`
b
`).o(`
c
`)
	);
	const o = {
		p: v
			.m(r)
			.o(`
a
`)
			.o(`
b
`).o(`
c
`)
	};

	// Controls: a call ending the chain takes every link onto a chain line; a short head;
	// templates on their own lines; one-line templates
	v.m(r)
		.o(
			`
a
`
		)
		.o(
			`
b
`
		)
		.o(
			`
c
`
		)
		.p(x);
	v.o(`
a
`).o(`
b
`).o(`
c
`);
	this.o(`
a
`).o(`
b
`).o(`
c
`);
	v.m(r)
		.o(
			`
a
`
		)
		.o(
			`
b
`
		)
		.o(
			`
c
`
		);
	v.m(r).o(`a`).o(`b`).o(`c`);
</script>

{v
	.m(r)
	.o(`
a
`)
	.o(`
b
`).o(`
c
`)}
<div
	data-attr={v
		.m(r)
		.o(`
a
`)
		.o(`
b
`).o(`
c
`)}
></div>
