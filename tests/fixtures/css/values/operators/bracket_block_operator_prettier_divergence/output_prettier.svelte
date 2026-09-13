<style>
	div {
		/* a `[…]` block is word text, not a value to descend into, so it is kept exactly
		as written — an operator byte inside it is content like any other */
		a: --modifier(2.5, [ *]);
		b: f(2.5, [ *]);
		c: [ *];
		d: f(2.5, [ * a]);
		e: f(2.5, [a *]);
		f: f(2.5, [ +]);
		g: f(2.5, [ * b *]);
		/* the cells that bound it: a block whose content is not an operator byte */
		h: f(2.5, [a]);
		i: f(2.5, [1.50]);
		j: f(2.5, [a-b]);
		k: f(2.5, [/]);
		/* prettier's space lands at every word boundary its tokenizer finds: the one gap it
		misses is the one before a group's LAST node, so a block that ends the value reads
		`[ *]` where one with a member after it reads `[ * ]` */
		l: a[ * ](2.5);
		/* the tokenizer ends a word at a `:` as well, wherever a `:` is a value token at
		all — inside a function's arguments, and at a custom property's top level */
		m: f(2.5, [a: b]);
		--n: [a: b];
		--o: a[b: c]d;
	}
</style>
