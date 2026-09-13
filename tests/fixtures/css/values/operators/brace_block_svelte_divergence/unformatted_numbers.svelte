<style>
	div {
		/* a `{…}` block inside a custom property's value: each brace is a member of its
		own, always glued to the block's interior and glued outward only as authored, and
		the members between the braces normalize */
		--a: a{1.50}c(2.50);
		--b: a{b}c(2.50);
		--c: a {1.50} c(2.50);
		--d: a{1.50} c(2.50);
		--e: a {1.50}c(2.50);
		--f: a{1.50 2.50}c;
		--g: a{1.50 * 2.50}c;
		--h: a{{1.50}}c;
		--i: a{1.50}{2.50};
		--j: a{}c(2.50);
		--k: f(a{1.50}c);
		/* an operator beside a brace reads it as a word: glued to the brace as authored,
		spaced from the operand */
		--l: a{1.50}/ 2.50;
		--m: a *{1.50}c;
		/* control: a `[…]` block is word text and is kept as written */
		--n: a[1.50]c(2.50);
		/* a top-level `:` between the braces is a token of its own there too: no space
		before it, one after */
		--o: a{b: 1.50}c;
	}
</style>
