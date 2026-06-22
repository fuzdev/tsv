<script lang="ts">
	// short — chain and breaking arg both fit on one line
	const a = obj.fn1(g, 'f').fn2(ok);

	// 100 — the flat chain prefix fits, so the chain stays flat and the last call's
	// breaking `new` arg expands; the chain does NOT break per method
	const chainAaaa = obj.fn1(scope, 'pathValueSegmentLiteralAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa').fn2(
		new Ctor(serialize(input), {
			status: 200,
			headers: defaultHeaders
		})
	);

	// 101 — the flat chain prefix is one char longer, so it overflows and the chain
	// breaks per method; the last call's `new` arg still expands
	const chainAaaaa = obj
		.fn1(scope, 'pathValueSegmentLiteralAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa')
		.fn2(
			new Ctor(serialize(input), {
				status: 200,
				headers: defaultHeaders
			})
		);
</script>
