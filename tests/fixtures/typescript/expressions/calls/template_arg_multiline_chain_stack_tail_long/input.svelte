<script lang="ts">
	// A lookup after a stack of template links, at the 100/101 boundary. In a statement it
	// drops to its own line at 101
	v.m(r).o(`
a
`).o(`
b
`).qq.ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;
	v.m(r).o(`
a
`).o(`
b
`).qq
		.pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;

	// An assignment target and a `new` callee inline every lookup down to the call, through
	// `!`, and the line overflows at 101 instead; an optional chain under `new` does not
	v.m(r).o(`
a
`).o(`
b
`).ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
	v.m(r).o(`
a
`).o(`
b
`).pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
	v.m(r).o(`
a
`).p.ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
	v.m(r).o(`
a
`).p.pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
	v.m(r).o(`
a
`)!.p.pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
	v.m(r).o(`
a
`)!.p.ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
	v.m(r).o(`
a
`).o(`
b
`).p.ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
	v.m(r).o(`
a
`).o(`
b
`).p.pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
	new (v.m(r).o(`
a
`).o(`
b
`).ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp)();
	new (v.m(r).o(`
a
`).o(`
b
`).pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp)();
	new (v.m(r).o(`
a
`).p.ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp)();
	new (v.m(r).o(`
a
`).p.pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp)();
	new (v?.m(r).o(`
a
`).o(`
b
`).qq.pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp)();
	new (v?.m(r).o(`
a
`).o(`
b
`).qq
		.ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp)();

	// A lookup off the call itself inlines under a declaration or an assignment, through `!`
	// on either side of it and `?.` alike; one off another lookup keeps its break point
	const y1 = v.m(r).o(`
a
`).o(`
b
`).pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;
	const y2 = v.m(r).o(`
a
`).o(`
b
`).ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;
	const y3 = v.m(r).o(`
a
`).o(`
b
`)!.ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;
	const y4 = v.m(r).o(`
a
`).o(`
b
`)!.pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;
	x = v?.m(r).o(`
a
`).o(`
b
`).pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;
	x = v?.m(r).o(`
a
`).o(`
b
`).ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;
	const y7 = v.m(r).o(`
a
`).o(`
b
`).ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp!;
	const y8 = v.m(r).o(`
a
`).o(`
b
`).pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp!;
	x = v.m(r).o(`
a
`).o(`
b
`).ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp!;
	x = v.m(r).o(`
a
`).o(`
b
`).pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp!;
	const y9 = v.m(r).o(`
a
`).o(`
b
`).pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp!!;
	const y10 = v.m(r).o(`
a
`).o(`
b
`).ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp!!;
	x = v.m(r).o(`
a
`).o(`
b
`)!.pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp!;
	x = v.m(r).o(`
a
`).o(`
b
`)!.ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp!;
	const y5 = v.m(r).o(`
a
`).o(`
b
`).pp.ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;
	const y6 = v.m(r).o(`
a
`).o(`
b
`).pp
		.pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;
	x = v.m(r).o(`
a
`).pp.ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;
	x = v.m(r).o(`
a
`).pp
		.pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp;

	// A computed lookup breaks inside its brackets
	v.m(r).o(`
a
`).o(`
b
`)[ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp];
	v.m(r).o(`
a
`).o(`
b
`)[
		pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp
	];

	// A parenthesized optional chain is the object of the lookups after it, not one of their
	// links, so they inline as an assignment target's
	(v?.m(r).o(`
a
`)).pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
	(v?.m(r).o(`
a
`)).ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
	(v?.o(`
a
`)).o(`
b
`).ppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
	(v?.o(`
a
`)).o(`
b
`).pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp =
		1;
</script>
