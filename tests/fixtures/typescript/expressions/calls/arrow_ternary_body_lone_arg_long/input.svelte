<script lang="ts">
	// a lone ternary-body arrow argument: broken out onto its own line, the arrow is printed
	// whole - its `=>` line counts the space after `=>`, so 99 chars fit and 100 do not
	// call, the `=>` line at 99 chars - the body breaks below `=>`
	fn(
		(attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			a ? b : c
	);
	// call, the `=>` line at 100 chars - the parameters break and the body stays in parens
	fn(
		(
			atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) => (a ? b : c)
	);
	// member call
	obj.fn(
		(attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			a ? b : c
	);
	obj.fn(
		(
			atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) => (a ? b : c)
	);
	// member chain
	a.b().c(
		(attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			a ? b : c
	);
	a.b().c(
		(
			atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) => (a ? b : c)
	);
	// `new`
	new Fn(
		(attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			a ? b : c
	);
	new Fn(
		(
			atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) => (a ? b : c)
	);
	// a typed signature, the `): R =>` line at 99 chars
	fn(
		(attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt: T): R =>
			a ? b : c
	);
	// a typed signature, the `): R =>` line at 100 chars - the parameters break
	fn(
		(
			atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt: T
		): R => (a ? b : c)
	);
	// the parameters break and the body is too wide for the `) =>` line - the body breaks too
	fn(
		(
			atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) =>
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
				? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
				: ccccccccccccccccccccccccccc
	);
	// a body with a forced break: hugged while the `callee((sig) =>` line is 99 chars
	fn((atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
		a
			? `x
y`
			: c
	);
	// a body with a forced break: the `callee((sig) =>` line at 100 chars - the argument breaks out
	fn(
		(attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			a
				? `x
y`
				: c
	);
	// `new`, the `callee((sig) =>` line at 99 chars, then at 100 chars
	new Fn((atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
		a
			? `x
y`
			: c
	);
	new Fn(
		(attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			a
				? `x
y`
				: c
	);
	// an object branch, a member call and a member chain - broken out
	fffffffffffffffffffffffffffffffffffff(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			a
				? {
						k: 1
					}
				: c
	);
	obj.fffffffffffffffffffffffffffffffffffff(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			a
				? `x
y`
				: c
	);
	a.b().fffffffffffffffffffffffffffffffffffff(
		(attttttttttttttttttttttttttttttttttttttttttttttttt) =>
			a
				? `x
y`
				: c
	);
	// broken out, the parameters break too
	fn(
		(
			atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) =>
			a
				? `x
y`
				: c
	);
	// a declaration and an assignment keep the call on the `=` line
	const v = fffffffffffffffffffffffffffffffffffff(
		(attttttttttttttttttttttttttttttttttttttttttttttttt) =>
			a
				? `x
y`
				: c
	);
	v = fffffffffffffffffffffffffffffffffffff(
		(atttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			a
				? `x
y`
				: c
	);
	// an argument that fits stays on the call's line
	fn((a) => (a ? b : c));
	// the same arrow as a middle argument is printed whole
	fn(
		x,
		(
			atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) => (a ? b : c),
		y
	);
	fn(
		x,
		(
			atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) =>
			a
				? `x
y`
				: c,
		y
	);
</script>
