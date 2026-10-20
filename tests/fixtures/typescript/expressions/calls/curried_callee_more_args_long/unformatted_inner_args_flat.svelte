<script lang="ts" module>
	// A curried call whose inner call takes more arguments than the outer one: the inner
	// argument list breaks first. At exactly 100 the whole call stays on one line.
	fn(aaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccc)(ddddddddddddddd);

	// At 101 the inner arguments break out and the outer list stays on the `)(` line.
	fn(aaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccc)(
		dddddddddddddddd
	);

	// A break forced in the outer list breaks the inner list too: a multi-line object, a
	// block-bodied arrow, a multi-line template.
	fn(a, b)({
		c: 1
	});
	fn(a, b)((c) => {
		return c;
	});
	fn(a, b)(`c
d`);

	// The same with type arguments, an optional inner call, an optional outer call and a
	// member callee.
	fn<T>(a, b)({
		c: 1
	});
	fn?.(a, b)({
		c: 1
	});
	fn(a, b)?.({
		c: 1
	});
	obj.fn(a, b)({
		c: 1
	});
	// A longer member chain keeps the inner list on the chain's line, and so does a member chain
	// before a sole multi-line template.
	obj.a().b().fn(x, y)({
		z: 1
	});
	obj.a().fn(a, b)(`c
d`);

	// A member callee over the width breaks the inner list the same way.
	obj.fnnnnnnnnnnnnn(aaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)(
		ccccccccccccccccccc
	);

	// So does a chain of two calls after a `this` or a capitalised head; after any other head
	// the inner list stays on the chain's line.
	this.a().fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx, yyyyyyyyyyyyyyyyyyyyyyy)(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	);
	Obj.a().fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx, yyyyyyyyyyyyyyyyyyyyyyy)(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	);
	z.a().fnnnnnnnnnnnnn(xxxxxxxxxxxxxxxxxxxxxxx, yyyyyyyyyyyyyyyyyyyyyyy)(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	);

	// Three levels: each call with more arguments than the call after it breaks before it.
	fnnnnnnnnnnnnnnnnnnnnn(aaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbb, ccccccccccccc)(
		dddddddddddddd,
		eeeeeeeeeeeee
	)(ggggggggggg);

	// Three levels where the inner prefix fits: only the last list breaks.
	fnnnn(aaaaaa, bbbbbbb, cccccc)(ddddddd, eeeeeee)(
		gggggggggggggggggggggggggggggggggggggggggggggggggggg
	);

	// A curried call followed by a member call breaks the inner list and keeps `.then(` on the
	// `)(` line, a long argument there included.
	fnnnnnnnnnnnnn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbb)(
		cccccccccccccccccccc
	).then(d);
	fnnn(aaaaaaa, bbbbbbb)(ccccc).then(
		dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
	);

	// The outer list hugging a trailing callback.
	fnnnn(aaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbb, c)(dddddddddddddddd, () => {
		return e;
	});

	// A short member callee such as `test.each` breaks the inner list the same way; one whose
	// inner call takes no more arguments keeps it.
	test.each(a, b, c)('name', (d) => {
		return d;
	});
	test.each(a, b)('name', (c) => {
		return c;
	});

	// Positions: a declarator, `export default`, an assignment, a return, an argument, an
	// object property value.
	const aaaaaaaaaaaaaaaa = connect(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccc)(
		Comp
	);
	export default connect(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccc)(
		Comp
	);
	a = fn(b, c)({
		d: 1
	});
	function fn1() {
		return fn(a, b)({
			c: 1
		});
	}
	fn2(
		fn(a, b)({
			c: 1
		})
	);
	obj = {
		a: fn(b, c)({
			d: 1
		})
	};

	// A block comment in the inner list breaks it the same way.
	const v = fnnn(aaaaaaa, bbbbbbb /* c */)(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	);

	// Behind a member or a non-null assertion the inner list breaks the same way.
	const v1 = obj.fn(aaaaaaa, bbbbbbb)(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	).p;
	const v2 = obj.fn(aaaaaaa, bbbbbbb)(
		zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
	)!;

	// A block-bodied callback in the outer list breaks the inner list; the signature stays on
	// the `)(` line, and one too long for it breaks its parameters on a line of its own.
	memoize(1000, a)((b: string) => {
		return [b];
	});
	let aaaaaaaa = memoize(1000, a)(
		(
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: B,
			ccccccccccccccccccccccccc: string,
			d: boolean
		): string[] => {
			return [c];
		}
	);

	// The inner list stays on the call's line when the inner call takes no more arguments than
	// the outer one, or the outer call takes none.
	fn(a)({
		b: 1
	});
	fn(a)(b, {
		c: 1
	});
	fnnnnnnnnnnnnn(aaaaaaaaaaaaaaaaaaaaaaa, b)(
		cccccccccccccccccccccccccccccccccccccccccc,
		dddddddddddddd
	);
	fnnn(aaaaaaa, bbbbbbb)().then(
		dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
	);

	// It stays there too under `new`, with a non-null assertion between the calls, when the
	// inner call hugs its last argument, and when the inner call is a test call.
	new Fn(a, b)({
		c: 1
	});
	fn(a, b)!({
		c: 1
	});
	fn(a, { b: 1 })({
		c: 1
	});
	fnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, (b) => {
		return b;
	})(cccccccccccccccccc);
	it('name', () => {
		return 1;
	})(xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx);
</script>

<p>
	{fn(a, b)({
		c: 1
	})}
</p>
