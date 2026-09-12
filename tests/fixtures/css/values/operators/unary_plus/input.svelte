<style>
	div {
		/* a `+` opening the value welds onto the word after it — the sign is the word's
		own — and a function named that way still normalizes its arguments */
		a: +a(2.5);
		b: +a;
		c: +#FFF;
		d: +a * 1.5;
		e: +a, b;
		/* straight after an operator it welds too, across an authored gap or none */
		f: 1.5 / +a(2.5);
		g: 1.5 * +a;
		h: 1.5 - +a;
		i: + +a;
		j: f(1.5 / +a(2.5));
		/* anywhere else the `+` is an operator and takes an operator's gaps: after a
		member, at the head of a function's arguments or of a group, and after a comma */
		k: 1.5 + a(2.5);
		l: f(+ a(2.5));
		m: (+ a);
		n: f(a, + b);
		/* it welds onto a word only: a group, a string and an `@`-word keep it an operator */
		o: +(2.5);
		p: +'x';
		q: + @a;
		/* a signed number is a number, whatever precedes it */
		r: +1.5;
		s: 1.5 / +2.5;
		/* calc() prints its authored gap either way */
		t: calc(+a(2.5));
	}
</style>
