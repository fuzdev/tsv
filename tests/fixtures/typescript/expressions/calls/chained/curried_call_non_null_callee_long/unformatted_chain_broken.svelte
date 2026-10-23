<script lang="ts">
	// A `!` on a curried call whose run roots in a member chain: the chain ends at the innermost
	// call, as it does without the `!`, so it stays on one line and the outer argument list
	// breaks. At exactly 100 the whole declarator fits.
	const v1 = obj.a().b().fnnn(x)!(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);

	// At 101 the outer arguments break out and the chain stays on one line.
	const v2 = obj
		.a()
		.b()
		.fnnn(x)!(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);

	// The same as without the `!`.
	const v3 = obj.a().b().fnnn(x)(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	);

	// Not only width: a chain of more than two calls with an argument that is not trivial stays
	// on one line too.
	const v4 = obj
		.a()
		.b()
		.fnnn(x)!((q) => q);

	// Two curried calls, a `!` over a call on a `!` member, two `!`s, a `!` between two curried
	// calls, and an inner call taking more arguments.
	obj
		.a()
		.b()
		.fnnn(x)(y)!(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		);
	obj
		.a()
		.b()
		.fnnn!(x)!(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		);

	obj
		.a()
		.b()
		.fnnn(x)!!(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		);
	obj
		.a()
		.b()
		.fnnn(x)!(y)!(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		);
	obj
		.a()
		.b()
		.fnnn(x, y)!(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		);

	// A member after the call: on one line at 100, the outer arguments broken out at 101.
	const v5 = obj.a().b().fnnn(x)!(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz).p;
	const v6 = obj
		.a()
		.b()
		.fnnn(x)!(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz).p;

	// A leading block comment on the outer argument.
	obj
		.a()
		.b()
		.fnnn(x)!(
			/* c */ zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		);

	// Positions: a statement, an assignment, a return, an argument, a property value.
	obj
		.a()
		.b()
		.fnnn(x)!(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		);
	w = obj
		.a()
		.b()
		.fnnn(x)!(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		);
	function fn1() {
		return obj
			.a()
			.b()
			.fnnn(x)!(
				zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
			);
	}
	fn2(
		fn3(
			obj
				.a()
				.b()
				.fnnn(x)!(
					zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
				)
		)
	);
	const o = {
		k: obj
			.a()
			.b()
			.fnnn(x)!(
				zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
			)
	};

	// An authored blank line breaks the chain one call per line; the outer arguments then
	// print at the statement's indent.
	v8 = obj
		.a()
		.b()

		.fnnn(x)!(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		);

	// Controls: a chain of a single call, a plain callee, an outer callee that is a call, and a
	// short call.
	obj.fnnn(x)!(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	);
	f(x)!(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	);
	obj.a().b().fnnn(x)!(y)(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	);
	const v7 = obj.a().b().fnnn(x)!(y);
</script>

<p>
	{obj
		.a()
		.b()
		.fnnn(x)!(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		)}
</p>
