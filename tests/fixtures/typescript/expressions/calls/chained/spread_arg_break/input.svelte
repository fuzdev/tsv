<script>
	// 3+ calls with spread arg forces chain break
	fn(...a)
		.b()
		.c();
	fn(...items)
		.render()
		.trim();

	// 2 calls with spread stays inline
	fn(...a).b();

	// 3+ calls without spread stays inline when short
	fn(a).b().c();
	fn(a, b).c().d();

	// chain with spread as call arg
	x.push(
		fn(...a)
			.b()
			.c()
	);

	// spread in middle call
	a.b(...x)
		.c()
		.d();

	// spread array
	fn(...[1, 2])
		.b()
		.c();
</script>
