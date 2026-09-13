<style>
	div {
		/* `\@` is ident content like any other escape, so the name is one ident sequence
		and the `(` glued to it opens that function's argument list */
		a: a\@b (2.5);
		b: \@b (2.5);
		c: f(a\@b (2.5));
		/* the cells that bound it: an empty `@` after the escape, a name with no `(`, an
		authored gap before the `(`, and the hex spelling of the same character */
		d: a\@(2.5);
		e: a\@b;
		f: a\@b (2.5);
		g: a\40 b(2.5);
		/* a raw `@` opens an at-word, which is a member of its own on either side */
		h: a @b (2.5);
		/* the name's extent, read against a second function beside it: tsv closes `a\@b(2.5)`
		and starts a new member at `c(`, where prettier's split puts its gap one member earlier */
		i: a\@b (2.5) c(2.5);
	}
</style>
