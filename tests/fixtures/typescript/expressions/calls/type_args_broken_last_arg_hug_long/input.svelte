<script lang="ts">
	// type arguments broken by width keep the last-argument hug after the `>(`
	// object, the `(` at 100 chars - the arguments break
	const a01 = fn<Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb>(
		{ a: 1 }
	);
	// object, the `(` at 101 chars - the type arguments break
	const a02 = fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>({ a: 1 });
	// arrow, the `(` at 100 chars - the arguments break
	const a03 = fn<Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb>(
		(a) => {
			fn1(a);
		}
	);
	// arrow, the `(` at 101 chars - the type arguments break
	const a04 = fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>((a) => {
		fn1(a);
	});
	// array, the `(` at 100 chars - the arguments break
	const a05 = fn<Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb>(
		[1, 2]
	);
	// array, the `(` at 101 chars - the type arguments break
	const a06 = fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>([1, 2]);
	// function, the `(` at 100 chars - the arguments break
	const a07 = fn<Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb>(
		function () {
			fn1();
		}
	);
	// function, the `(` at 101 chars - the type arguments break
	const a08 = fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>(function () {
		fn1();
	});

	// `new`
	const a09 = new Fn<Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb>(
		{ a: 1 }
	);
	const a10 = new Fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>({ a: 1 });
	const a11 = new Fn<Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb>(
		(a) => {
			fn1(a);
		}
	);
	const a12 = new Fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>((a) => {
		fn1(a);
	});

	// a leading argument before the hugged one
	const a13 = fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>(x, { a: 1 });
	const a14 = new Fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>(x, (a) => {
		fn1(a);
	});

	// positions
	fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>({ a: 1 });
	a = fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>({ a: 1 });
	function fn2() {
		return fn<
			Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		>({ a: 1 });
	}
	fn3(
		fn<
			Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		>({ a: 1 })
	);
	fn3(
		x,
		fn<
			Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		>((a) => {
			fn1(a);
		})
	);
	const a15 = () =>
		fn<
			Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		>({ a: 1 });
	async function fn4() {
		await fn<
			Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		>({ a: 1 });
		const a16 = await fn<
			Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		>({ a: 1 });
	}
	const a17 = {
		b: fn<
			Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		>({ a: 1 })
	};
	const a18 = cond
		? fn<
				Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
				Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			>({ a: 1 })
		: null;

	const a19 = fn?.<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>({ a: 1 });
	const a20 = fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>({ a: 1 }) as T;
	class C {
		@dec<
			Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		>({ a: 1 })
		m() {}
	}

	// type arguments broken by an authored-expanded type literal: the hug is measured from the `}>(`
	// arrow, the `{` at 100 chars - the hug stays
	const a21 = fn<{
		a: A;
	}>('bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', (a) => {
		fn1(a);
	});
	// arrow, the `{` at 101 chars - the arguments break
	const a22 = fn<{
		a: A;
	}>(
		'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
		(a) => {
			fn1(a);
		}
	);
	// object, the `{` at 101 chars - the arguments break
	const a23 = fn<{
		a: A;
	}>(
		'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
		{
			a: 1
		}
	);

	// the other argument ladders after type arguments broken by width
	fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>(() => {
		fn1();
	}, 100);
	fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>(x, (a) =>
		// c
		g(a)
	);
	fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>((a) =>
		// c
		({ a: 1 })
	);
	fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>((a) => g(a));
	fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>((a) => (a ? b : c));
	fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>()('x', () => {
		fn1();
	});

	// controls: a plain argument, the member-call spelling, and a lone nullable union type argument
	const a24 = fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>(x);
	const a25 = b.fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>({ a: 1 });
	const a26 =
		fn<Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | null>({
			a: 1
		});
	fn<Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | null>(
		{ a: 1 }
	);
</script>

{fn<
	Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
>({ a: 1 })}

<div
	data-attr={fn<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>({ a: 1 })}
></div>
