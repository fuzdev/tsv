<script lang="ts">
	// Member-chain call: when lhs + " = a.fn(" ≤ print_width, expand call args (not break at =)
	// Applies to call expressions with member callee (e.g. vi.mocked, a.fn)

	// AwaitExpression
	const { aaaa_bbbb_cccc_dddd, eeee_ffff_gggg_hhhh } = a.fn(
		await import('./aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa')
	);

	// UnaryExpression: typeof
	const { aaaa_bbbb_cccc_1111, eeee_ffff_gggg_2222 } = a.fn(
		typeof some_very_long_variable_name_that_makes_it_overflow
	);

	// TSAsExpression
	const { aaaa_bbbb_cccc_3333, eeee_ffff_gggg_4444 } = a.fn(
		something_long as SomeVeryLongTypeNameThatOverflows
	);

	// TSNonNullExpression
	const { aaaa_bbbb_cccc_5555, eeee_ffff_gggg_6666 } = a.fn(
		some_very_long_variable_name_that_makes_the_line_overflow!
	);

	// ImportExpression (without await)
	const { aaaa_bbbb_cccc_7777, eeee_ffff_gggg_8888 } = a.fn(
		import('./aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa')
	);

	// Contrast: lhs + " = a.fn(" = 103 cols (> 100) → break at =
	const { aaaa_bbbb_cccc_dddd_eeee_ffff_gggg_hhhh_iiii, jjjj_kkkk_llll_mmmm_nnnn_oooo_pppp_qqqq } =
		a.fn(x, y);
</script>
