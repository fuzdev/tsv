<style>
	div {
		/* a custom property's value is parsed, so a top-level `:` in it is a token of its
		own: it takes no space before and one after, and both sides normalize */
		--a: a: 1.50;
		--b: a: b;
		--c: a: b: 1.50;
		--d: 1.50: 2.50;
		/* a second colon is a member of its own too, so the pair keeps one space apiece */
		--e: a: : 1.50;
		/* the rule reads across a whitespace and a comma element boundary alike */
		--f: a: 1.50 b: 2.50;
		--g: a: 1.50, b: 2.50;
		/* a `:` inside a group is the group's own token, custom property or not */
		--h: f(a: 1.50);
		/* a url() argument is opaque, so its `:` is content */
		--i: url(a:1.50);
		/* a colon with nothing before it is the same token: no space before, one after */
		--j: : a;
		/* a `+` the author glued to the digits after the colon is the number's own sign */
		--k: a: +1.50;
		/* an `@`-word takes a `:` as its own content, so the value is verbatim on both sides */
		--l: @a:1.50;
		/* a custom-property-shaped word ahead of the colon is a word like any other */
		--m: --y: 1.50;
		/* a `+` after the colon is an operator: a colon is no operator for the sign to
		weld onto, so it keeps a space on each side — where the digits above make the
		same `+` the number's own sign */
		--n: a: + b;
		--o: : + a;
		/* a `-` straight before the colon is an operator too — the colon opens no word for
		it to head — so the pair glues and the colon's own space follows */
		--p: +-: a;
	}
</style>
