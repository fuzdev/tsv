<script lang="ts">
	// A leading own-line comment on a branch must not drop the ternary TEST's parens.

	// Arrow test — parens are load-bearing: without them `() => x ? a : b` reparses
	// as `() => (x ? a : b)` (the arrow body absorbs the whole ternary)
	const a = (() => x)
		? // c
			aaaaaaaaaaaaaaaaaaa
		: bbbbbbbbbbbbbbbbbbb;

	// `as` cast test — clarity parens (same AST)
	const b = (x as T)
		? // c
			aaaaaaaaaaaaaaaaaaa
		: bbbbbbbbbbbbbbbbbbb;
</script>
