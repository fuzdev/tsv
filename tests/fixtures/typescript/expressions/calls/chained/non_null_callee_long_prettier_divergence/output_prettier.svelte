<script lang="ts">
	// A call on a non-null callee is a link of its member chain, laid out as the same call
	// without the `!`: the chain stays on one line while it fits (exactly 100 here)
	const v1 = obj.a().b().fnnn!(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);

	// and breaks one call per line past it (101)
	const v2 = obj.a().b().fnnn!(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	);

	// the same as without the `!`
	const v3 = obj
		.a()
		.b()
		.fnnn(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);

	// Mid-chain: the call on a non-null callee is one link among the others
	obj.a().b().fnnn!(x)
		.c()
		.d(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);
	obj
		.a()
		.b()
		.fnnn(x)
		.c()
		.d(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);

	// Not only width: a chain of more than two calls with an argument that is not trivial breaks
	// one call per line at any width, with the `!` as without it
	const v4 = obj.a().b().fnnn!((q) => q);
	const v5 = obj
		.a()
		.b()
		.fnnn((q) => q);
	obj.a().b().fnnn!(x).c((q) => q);
	obj
		.a()
		.b()
		.fnnn(x)
		.c((q) => q);

	// A commented empty-bodied block arrow fits on the last call's line once the chain breaks (101)
	const v6 = a.b().fnzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz!(
		/* l */ (q) => {}
	);
	const v7 = a
		.b()
		.fnzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz(/* l */ (q) => {});

	// A `!` on a curried call mid-chain is a link too
	obj.a().b().fnnn(x)!(y)
		.c()
		.d(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);
	obj
		.a()
		.b()
		.fnnn(x)(y)
		.c()
		.d(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);

	// Positions: an assignment, a return, an argument
	w = obj.a().b().fnnn!(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	);
	function fn1() {
		return obj.a().b().fnnn!(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		);
	}
	fn2(
		obj.a().b().fnnn!(
			zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
		)
	);
</script>

{obj.a().b().fnnn!(x)
	.c()
	.d(zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz)}
