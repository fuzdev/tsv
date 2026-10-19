<script lang="ts">
	// a multiline tagged-template callee: the arguments are measured from the closing line
	// the hugged head at 100 chars
	fn`
	a | b
	${1} | ${2}
`('abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', ({ a, b }) => {
		fn1(a, b);
	});
	// the hugged head at 101 chars
	fn`
	a | b
	${1} | ${2}
`(
		'abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
		({ a, b }) => {
			fn1(a, b);
		}
	);
	fn`
	a | b
	${1} | ${2}
`('abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', function () {
		fn1();
	});
	fn`
	a | b
	${1} | ${2}
`(
		'abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
		function () {
			fn1();
		}
	);
	// `new`, an async arrow
	new Fn`
	a | b
	${1} | ${2}
`(
		'abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
		async ({ a, b }) => {
			fn1(a, b);
		}
	);

	// positions
	const a01 = fn`
	a | b
	${1} | ${2}
`(
		'abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
		({ a, b }) => {
			fn1(a, b);
		}
	);
	async function fn2() {
		await fn`
	a | b
	${1} | ${2}
`(
			'abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
			({ a, b }) => {
				fn1(a, b);
			}
		);
		return fn`
	a | b
	${1} | ${2}
`(
			'abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
			({ a, b }) => {
				fn1(a, b);
			}
		);
	}
	fn3(
		fn`
	a | b
	${1} | ${2}
`(
			'abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
			({ a, b }) => {
				fn1(a, b);
			}
		)
	);
	const a02 = () =>
		fn`
	a | b
	${1} | ${2}
`(
			'abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
			({ a, b }) => {
				fn1(a, b);
			}
		);
	const a03 = {
		c: fn`
	a | b
	${1} | ${2}
`(
			'abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
			({ a, b }) => {
				fn1(a, b);
			}
		)
	};

	// a test table
	it.each`
		a    | b
		${1} | ${2}
	`(
		'returns $b when the value is $a and the other options are left at their defaults',
		({ a, b }) => {
			expect(fn(a)).toBe(b);
		}
	);

	// a curried call whose callee call breaks: the arguments are measured from its closing line
	// the callee's arguments broken by width, the hugged head at 100 chars
	f(
		'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
		'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'
	)('ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', () => {
		fn1();
	});
	// the callee's arguments broken by width, the hugged head at 101 chars
	f(
		'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
		'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'
	)(
		'tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		() => {
			fn1();
		}
	);
	// a function argument, the hugged head at 100 chars
	fn(function () {
		return 2;
	})('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', () => {
		fn1();
	});
	// a function argument, the hugged head at 101 chars
	fn(function () {
		return 2;
	})(
		'ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		() => {
			fn1();
		}
	);
	// a multiline template argument, the hugged head at 100 chars
	fn(`
	a | b
`)('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', () => {
		fn1();
	});
	// a multiline template argument, the hugged head at 101 chars
	fn(`
	a | b
`)(
		'ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		() => {
			fn1();
		}
	);

	// controls: a head that fits, an object, a one-line template, a broken non-template callee
	fn`
	a | b
	${1} | ${2}
`('text', ({ a, b }) => {
		fn1(a, b);
	});
	fn`
	a | b
	${1} | ${2}
`('abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', {
		aaaaaaaaaa,
		bbbbbbbbbb
	});
	fn`a | b`(
		'abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
		({ a, b }) => {
			fn1(a, b);
		}
	);
	fn({
		a: 1
	})('abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', ({ a, b }) => {
		fn1(a, b);
	});
</script>

{fn`
	a | b
	${1} | ${2}
`(
	'abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
	({ a, b }) => {
		fn1(a, b);
	}
)}
