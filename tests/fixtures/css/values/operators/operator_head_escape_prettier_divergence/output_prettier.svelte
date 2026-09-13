<style>
	div {
		/* a head `+` glues to the member after it only where that member is not word-shaped,
		and prettier decides word-shaped-ness from the escape's SPELLING rather than from the
		token the escape spells: `\61` is the ident `a`, and only the escaped spelling loses
		its gap. The discriminator is a DECIMAL digit after the `\`, wherever in the token it
		sits — `\5a` spells `Z`, and it is the leading `5` that glues it */
		a: +\61;
		b: +a\61;
		c: +\5a;
		/* the cells that bound it: an escape whose `\` is followed by no decimal digit stays
		word content to prettier's value parser and keeps its gap — `\e9` is the cell that
		tells that rule apart from a HEX one, spelling `é` out of hex digits alone — and so
		does the plain word the first cell spells */
		d: + \e9;
		e: + \?;
		f: + a\z;
		g: + a;
		/* the arm is a HEAD arm: with no operator at all, or with an operand on the
		operator's left, the escape is an ordinary member on both sides */
		h: a \61;
		i: 1.5 / \61;
	}
</style>
