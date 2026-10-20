<script lang="ts">
	// A member chain whose last call is curried: the chain ends at the inner call, so it stays
	// on one line and the outer argument list breaks. At exactly 100 the whole call fits.
	obj.a().b().fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);

	// At 101 the outer arguments break out and the chain stays on one line.
	obj
		.a()
		.b()
		.fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);

	// The same with more outer arguments, a block-bodied callback, three curried calls, an
	// optional outer call, a computed member and a `this` base.
	obj
		.a()
		.b()
		.fnnn(x)(yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy, zzzzzzzzzzzzzzzzzzzzzzzzzzzzz);
	obj
		.a()
		.b()
		.fnnn(x)((y) => {
			return y;
		});
	obj
		.a()
		.b()
		.fnn(xxxxxxxxxxxxxxxxxxx)(yyyyyyyyyyyyyyyyy)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);
	obj
		.a()
		.b()
		.fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx)?.(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);
	obj
		.a()
		.b()
		[k](xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);
	this.a()
		.b()
		.fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);

	// Positions: a declarator, a return, an argument.
	const v = obj
		.a()
		.b()
		.fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);
	function fn1() {
		return obj
			.a()
			.b()
			.fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);
	}
	fn2(
		obj
			.a()
			.b()
			.fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz)
	);

	// An inner call with more arguments than the outer one keeps its list on the chain's line
	// while the chain fits there.
	obj
		.a()
		.b()
		.fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx, yyyyyyyyyyyyyyyyyyyyyyyyy)(zzzzzzzzzzzzzzzzzzzzzzzzzz);
	obj
		.a()
		.b()
		.fnnn(xxxxxxxxxxx, yyyyyyyyyy)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);
	obj
		.a()
		.fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx, yyyyyyyyyyyyyyyyyyyyyyy)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		);

	// When the chain breaks one call per line, an inner call with more arguments than the
	// outer one breaks its list.
	obj
		.a((x) => x)
		.b((y) => y)
		.fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxx, yyyyyyyyyyyyyyyyyyyyy)(zzzzzzzzzzzzzzzzzzzzz);

	// An outer argument that breaks then prints at the statement's indent.
	obj
		.a((x) => x)
		.b((y) => y)
		.fn(a, b)({
			z: 1
		});

	// A broken chain keeps a curried tail whose inner call takes no more arguments on one line.
	obj
		.a((x) => x)
		.b((y) => y)
		.fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxx)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);

	// The chain breaks one call per line when its head alone overflows.
	objjjjjjjjjjjjjjjjjjjjjj
		.aaaaaaaaaaaaaaaaaaaaaaa()
		.bbbbbbbbbbbbbbbbbbbbbb()
		.fnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(x)(z);

	// A forced break in the outer list keeps the chain on one line; so does a chain of members
	// with a single call.
	obj.a().b().fn(x)({
		z: 1
	});
	obj.a.fnnnnnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx)(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	);

	// A member callee breaks its inner list for a multi-line template in the outer list, and
	// keeps it on one line when the call fits.
	obj.fn(a, b)(`c
d`);
	obj.fn(a, b)(c);
</script>

<p>
	{obj
		.a()
		.b()
		.fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz)}
</p>
