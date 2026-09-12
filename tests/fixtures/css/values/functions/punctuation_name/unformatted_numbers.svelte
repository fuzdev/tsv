<style>
	a {
		/* the value layer's oracle is postcss-values-parser, whose word ends at a small set
		of characters — a `/` is not one of them, so `aa/` is a function NAME and its
		arguments normalize like any other function's */
		b: aa/(1.50);
		c: aa/f(1.50, 2.50);
		d: aa/bb/(1.50);
		/* the name is emitted as written, hex-looking or not */
		e: #FFF/(1.50);
		/* every other character the word admits, each measured against the oracle */
		f: aa!(1.50) aa#(1.50) aa$(1.50) aa%(1.50) aa&(1.50);
		g: aa.(1.50) aa<(1.50) aa=(1.50) aa>(1.50) aa?(1.50);
		h: aa](1.50) aa^(1.50) aa|(1.50) aa~(1.50);
		/* one of them may also OPEN the name */
		i: .aa(1.50);
		j: ]aa(1.50);
		/* a `[…]` block's interior is word TEXT rather than a group the oracle descends
		into, so the whole region is the name and its own number is kept as written */
		k: a[bb]c(1.50);
		l: a[1.50]c(1.50);
		/* controls: a leading `/` is an operator, so the name that follows it is the
		ordinary ident one, and a quote or a dangling `\` is no word character at all */
		m: /aa(1.50);
		n: a\(b(.10);
	}
</style>
