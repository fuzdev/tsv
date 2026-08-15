<script>
	// a zero-parameter block arrow followed by an array literal stays flat, whatever
	// the callee is named
	useEffect(() => {
		a();
	}, [b, c]);

	fn(() => {
		a();
	}, [b, c]);

	new A(() => {
		a();
	}, [b, c]);

	obj.fn1(1).fn2(() => {
		a();
	}, [b, c]);

	import(() => {
		a();
	}, [b, c]);

	// the three-argument form takes an identifier first
	useImperativeHandle(ref, () => {
		a();
	}, [b, c]);

	// comments inside an argument leave the layout alone
	useEffect(() => {
		// c
		a();
	}, [/* c */ b, c]);

	// the deps array still breaks on its own width
	useEffect(() => {
		a();
	}, [
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		cccccccccccccccccccccccccccccccc,
		dddddddddddddddddddddddddddddd,
		eeeeeeeeeeeeeeeeeeeeeeee
	]);

	// control: a parameter on the callback breaks every argument out
	fn(
		(x) => {
			a();
		},
		[b, c]
	);

	// control: a comment attached to an argument breaks them out
	fn(
		/* c */ () => {
			a();
		},
		[b, c]
	);

	import(
		/* c */ () => {
			a();
		},
		[b, c]
	);

	// control: the three-argument form needs a plain identifier first
	useImperativeHandle(
		o.p,
		() => {
			a();
		},
		[b, c]
	);
	// an optional call takes the layout too — the shape is the whole rule
	obj?.fn1(() => {
		a();
	}, [b, c]);

	// control: a fourth argument is not the shape
	fn(
		() => {
			a();
		},
		[b, c],
		d
	);

	// control: the three-argument form needs an identifier, not any short expression
	fn(
		1,
		() => {
			a();
		},
		[b, c]
	);

	// control: a spread is not the callback
	fn(...[() => {}], [b, c]);
</script>
