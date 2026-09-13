<style>
	div {
		/* an operator with nothing before it binds to the member after it: at the head of
		the value it takes no gap, whatever that member is — as long as the glue leaves the
		two tokens apart (where it would merge them into one, the gap stands instead, which
		is the `operator_head_weld_prettier_divergence` sibling) */
		a: / 2.5;
		b: / a;
		c: / (2.5);
		d: - @a;
		e: - (2.5);
		f: - [a];
		g: - 'x';
		h: - +a;
		i: + (2.5);
		j: + 'x';
		/* the same head rule inside a function's arguments, after a comma and after
		another operator — each a position with no operand on the operator's left. Its
		reach is not the same at all three: at the value's head, inside a function and
		after a comma a head `/` glues in front of every member kind, while after ANOTHER
		operator only a head `-` is unconditional — a `/` or a `+` there keeps its gap
		before a word (an `@`-word and a `[…]` block included) or a function, and glues
		only before a member that is neither */
		k: f(/ 2.5);
		l: f(1.5,   / 2.5);
		m: a,   / 2.5;
		n: 1.5 / / 2.5;
		o: 1.5   /   /   a;
		p: 1.5   /   /   f(2.5);
		q: 1.5   /   /   @a;
		r: 1.5   /   /   [a];
		/* `*` is the one operator that keeps its gap wherever it stands */
		s: *   2.5;
		/* a head `+` glues only where the member after it is not word-shaped: before a
		word, an `@`-word, a `[…]` block or a function it takes the ordinary member gap on
		both formatters, where a head `-` glues in front of every one of them (a word
		whose escape opens on a DECIMAL digit is where that agreement ends — the
		`operator_head_escape_prettier_divergence` sibling) */
		t: +   a;
		u: +   @a;
		v: +   [a];
		w: +   f(2.5);
		x: +   -a;
		/* an operator between two operands keeps the gaps its neighbours give it */
		y: 1.5   /   2.5;
		z: 1.5   *   2.5;
		aa: aa   /   2.5;
		/* a `-` straight after another operator is the operator itself wherever it opens
		neither an ident nor a number of its own — a string, a group, a `/`, a `*` — and
		the head rule above then binds it to what follows (a `[…]` block takes that same
		reading, where prettier's word does not — the `hyphen_word_extent_prettier_divergence`
		sibling) */
		ab: +- 'x';
		ac: +- (2.5);
		ad: +- / 2.5;
		ae: +-   *   a;
	}
</style>
