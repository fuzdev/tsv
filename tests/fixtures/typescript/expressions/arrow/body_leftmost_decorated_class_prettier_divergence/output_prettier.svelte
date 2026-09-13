<script lang="ts">
	// A concise body cannot START with `@`, so a DECORATED class expression at the body's
	// leftmost position takes a paren pair of its own — around the class alone, exactly as
	// a leftmost object literal does. The pair breaks open, since the decorator owns a line.
	const aaa = () =>
		@dec
		class {}.bbb;

	const ccc = () =>
		@dec
		class {};

	const ddd = () =>
		@dec
		class {}();

	const eee = () =>
		@dec
		class {}
			? fff
			: ggg;

	// A frozen body whose ROOT is the class takes the same pair — it is the position's, so
	// it rides outside the slice.
	const hhh = () =>
		// prettier-ignore

			@dec  class  {}
		;

	// A frozen COMPOSITE body — one whose leftmost node is the class rather than the body's
	// root — carries that pair INSIDE the slice: acorn's member and call spans begin at the
	// author's `(`, so the freeze takes the pair along with the class and prints it back
	// verbatim. The position-pair seam the expression statement also uses is what mints the
	// same form from the shell authoring, where the directive sits inside the erased parens.
	const ooo = () =>
		// prettier-ignore
		(@dec  class  {}).ppp;

	const qqq = () =>
		// prettier-ignore
		(@dec  class  {})();

	// An UNDECORATED class opens a concise body fine and stays bare, and a leftmost object
	// keeps the pair it always had.
	const iii = () => class {}.jjj;
	const kkk = () => ({ lll: 1 }).mmm;

	// At statement position the same leftmost rule already parenthesized the class.
	(
		@dec
		class {}
	).nnn;
</script>
