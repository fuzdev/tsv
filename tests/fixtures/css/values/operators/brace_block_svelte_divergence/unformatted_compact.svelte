<style>
	div {
		/* a `{…}` block inside a custom property's value: each brace is a member of its
		own, always glued to the block's interior and glued outward only as authored, and
		the members between the braces normalize */
		--a: a{1.5}c(2.5);
		--b: a{b}c(2.5);
		--c: a {1.5} c(2.5);
		--d: a{1.5} c(2.5);
		--e: a {1.5}c(2.5);
		--f: a{1.5 2.5}c;
		--g: a{1.5*2.5}c;
		--h: a{{1.5}}c;
		--i: a{1.5}{2.5};
		--j: a{}c(2.5);
		--k: f(a{1.5}c);
		/* an operator beside a brace reads it as a word: glued to the brace as authored,
		spaced from the operand */
		--l: a{1.5}/2.5;
		--m: a*{1.5}c;
		/* control: a `[…]` block is word text and is kept as written */
		--n: a[1.50]c(2.5);
	}
</style>
