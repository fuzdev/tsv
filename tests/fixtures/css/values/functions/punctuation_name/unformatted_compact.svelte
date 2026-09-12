<style>
	a {
		/* the value layer's oracle is postcss-values-parser, whose word ends at a small set
		of characters — a `/` is not one of them, so `aa/` is a function NAME and its
		arguments normalize like any other function's */
		b: aa/(1.5);
		c: aa/f(1.5,2.5);
		d: aa/bb/(1.5);
		/* the name is emitted as written, hex-looking or not */
		e: #FFF/(1.5);
		/* every other character the word admits, each measured against the oracle */
		f: aa!(1.5) aa#(1.5) aa$(1.5) aa%(1.5) aa&(1.5);
		g: aa.(1.5) aa<(1.5) aa=(1.5) aa>(1.5) aa?(1.5);
		h: aa](1.5) aa^(1.5) aa|(1.5) aa~(1.5);
		/* one of them may also OPEN the name */
		i: .aa(1.5);
		j: ]aa(1.5);
		/* a `[…]` block's interior is word TEXT rather than a group the oracle descends
		into, so the whole region is the name and its own number is kept as written */
		k: a[bb]c(1.5);
		l: a[1.50]c(1.5);
		/* controls: a leading `/` is an operator, so the name that follows it is the
		ordinary ident one, and a quote or a dangling `\` is no word character at all */
		m: /aa(1.5);
		n: a\(b(.10);
	}
</style>
