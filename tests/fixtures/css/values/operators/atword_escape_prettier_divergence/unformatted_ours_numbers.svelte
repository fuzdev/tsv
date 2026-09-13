<style>
	div {
		/* an `@`-word carries its escapes whole — a hex escape's optional whitespace
		terminator belongs to the escape — and the group after the word is a member of
		its own, so the member gap stands */
		a: @a\62 c (2.50);
		b: @a\62 c;
		c: @a\62 2c;
		d: @a\62 c d;
		e: @\62 c (2.50);
		/* an escape with no terminator to carry: only the member gap is in question */
		f: @a\41 (2.50);
		g: @a\g (2.50);
		/* controls: a plain `@`-word takes the member gap on both sides */
		h: @a (2.50);
		i: @ab (2.50);
		/* a `-` after the escape is at-word content, so the whole word is one member and the
		escape's terminator still belongs to the escape */
		j: @a\62 c-d (2.50);
	}
</style>
