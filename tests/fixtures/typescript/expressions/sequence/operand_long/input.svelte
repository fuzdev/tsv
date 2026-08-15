<script lang="ts">
	// 100 chars - stays inline
	const a = (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) as Foo;

	// 101 chars - the sequence breaks at its comma, like any other operand
	const b = (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) as Foo;

	// Non-null operand
	const c = (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbb)!;

	// Member object
	const d = (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
		.prop;

	// Unary operand
	const e = !(expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

	// Call callee
	const f = (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)(
		arg
	);

	// Await operand
	const g = await (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
</script>
