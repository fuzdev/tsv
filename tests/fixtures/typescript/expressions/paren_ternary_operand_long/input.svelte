<script lang="ts">
	// 100 chars — the whole cast fits, the ternary stays inline in its parens
	const a = (cccccccccccccccccccccccc ? aaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbbb) as Foo;

	// 101 chars — the parens expand and the ternary stays inline inside them
	const b = (
		cccccccccccccccccccccccc ? aaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbbbb
	) as Foo;

	// 101 chars — satisfies shares the cast path
	const c = (
		cccccccccccccccccccccc ? aaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbb
	) satisfies Foo;

	// 101 chars — a non-null assertion expands its operand parens the same way
	const d = (
		cccccccccccccccccccccccccc ? aaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbbbbbb
	)!;

	// Too long for the member to simply break: the parens expand, member hugs the `)`
	const e = (
		cccccccccccccccccccccccccccccc
			? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	)!.prop;

	// Same shape without the non-null assertion
	const f = (
		cccccccccccccccccccccccccccccc
			? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	).prop;

	// A unary argument is the boundary of the rule: it hangs instead of expanding
	const g = !(cccccccccccccccccccccccccc
		? aaaaaaaaaaaaaaaaaaaaaaaaaa
		: bbbbbbbbbbbbbbbbbbbbbbbbbbb);
</script>
