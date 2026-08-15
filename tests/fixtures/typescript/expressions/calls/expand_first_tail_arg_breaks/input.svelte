<script>
	// a tail argument that breaks defeats the inline tail, so every argument breaks out
	fn(
		() => {
			a();
		},
		fn1(
			// c
			b
		)
	);

	new A(
		() => {
			a();
		},
		fn1(
			// c
			b
		)
	);

	obj.fn1(1).fn2(
		() => {
			a();
		},
		fn1(
			// c
			b
		)
	);

	// control: a tail argument that stays flat keeps the expand-first hug
	fn(() => {
		a();
	}, b);

	new A(() => {
		a();
	}, b);

	obj.fn1(1).fn2(() => {
		a();
	}, b);
	// an inline block comment leading the tail argument rides with it, and a tail that
	// breaks still takes every argument out
	fn(() => {
		a();
	}, /* c */ b);

	new A(() => {
		a();
	}, /* c */ b);

	fn(
		() => {
			a();
		},
		/* c */ fn1(
			// c
			b
		)
	);

	new A(
		() => {
			a();
		},
		/* c */ fn1(
			// c
			b
		)
	);
</script>
