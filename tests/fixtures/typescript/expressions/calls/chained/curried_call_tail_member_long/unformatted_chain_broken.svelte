<script lang="ts">
	// A member, `!` or index after a curried call whose run roots in a member chain: the chain
	// ends at the innermost call, so it stays on one line and the outer argument list breaks.
	// At exactly 100 the whole declarator fits.
	const v1 = obj.a().b().fnnn(x)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz).p;

	// At 101 the outer arguments break out and the chain stays on one line.
	const v2 = obj
		.a()
		.b()
		.fnnn(x)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz).p;

	// Not only width: an argument that is not trivial expands a chain of more than two calls,
	// but the chain ends at the innermost call, so it stays on one line at any width.
	const v4 = obj
		.a()
		.b()
		.fnnn(x)((q) => q).p;

	// An object argument hugs the outer call's parentheses: on one line at 100, hugged at 101.
	const v5 = obj.a().b().fnnn(x)({ k: zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz }).p;
	const v6 = obj
		.a()
		.b()
		.fnnn(x)({ k: zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz }).p;

	// A `!`, an index, two members, an optional member, a member after a `!`.
	obj
		.a()
		.b()
		.fnnn(x)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		)!;
	obj
		.a()
		.b()
		.fnnn(x)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		)[0];
	obj
		.a()
		.b()
		.fnnn(x)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		).p.q;
	obj
		.a()
		.b()
		.fnnn(x)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		)?.p;
	obj
		.a()
		.b()
		.fnnn(x)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		)!.p;

	// More inner arguments, three curried calls, a `this` base.
	obj
		.a()
		.b()
		.fnnn(x, y)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		).p;
	obj
		.a()
		.b()
		.fnnn(x)(y)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		).p;
	this.a()
		.b()
		.fnnn(x)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		).p;

	// Positions: a statement, an assignment, a return, an argument, a property value.
	obj
		.a()
		.b()
		.fnnn(x)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		).p;
	w = obj
		.a()
		.b()
		.fnnn(x)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		).p;
	function fn1() {
		return obj
			.a()
			.b()
			.fnnn(x)(
				zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
			).p;
	}
	fn2(
		fn3(
			obj
				.a()
				.b()
				.fnnn(x)(
					zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
				).p
		)
	);
	const o = {
		k: obj
			.a()
			.b()
			.fnnn(x)(
				zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
			).p
	};

	// An authored blank line breaks the chain one call per line; the outer arguments then
	// print at the statement's indent.
	v3 = obj
		.a()
		.b()

		.fnnn(x)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		).p;

	// A call after the member joins the chain, which breaks one call per line.
	obj
		.a()
		.b()
		.fnnn(x)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		)
		.p();

	// A chain of a single call keeps its own layout.
	obj.fnnn(x)(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz)
		.p;
</script>

<p>
	{obj
		.a()
		.b()
		.fnnn(x)(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		).p}
</p>
