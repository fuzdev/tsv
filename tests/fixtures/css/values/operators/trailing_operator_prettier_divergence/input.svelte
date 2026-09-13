<style>
	div {
		/* an operator with no operand after it is the value's last member, and takes the
		gap its left neighbour gives it */
		a: 1.5 * -;
		b: 1.5 * +;
		c: 1.5 / -;
		/* a `/` beside it is the one gap the author keeps: the members on both sides of it
		are operators, neither is a word, so the division's own rule decides it and every
		authored spelling is a fixed point */
		d: 1.5/-;
		e: 1.5 /-;
		f: 1.5/ -;
		/* two operators and nothing else: the head one binds to the member after it, which
		here is the trailing operator */
		g: +-;
		h: ++;
		i: -+;
		j: /+;
		/* the bounding cells: the same runs inside a function's arguments, the one place
		prettier's parse succeeds and the two formatters agree cell for cell. A custom
		property is frozen exactly as a plain one is, so the two cells after them agree
		only because tsv's output is the frozen bytes */
		k: f(1.5 * -);
		l: f(1.5 /-);
		m: f(+-);
		--n: +-;
		--o: a +-;
		/* a custom property's `/` is the `font` shorthand's: a glued one beside a
		font-size operand stays glued, and that arm reads the gap on the operator's OWN
		left — so `--p` closes a gap the plain-property `f` above keeps open, and the two
		cells after it are where the arm does not reach */
		--p: 1.5/-;
		--q: 1.5 / -;
		--r: a/ -;
		/* the run with an operand after it, and a lone trailing operator with no operator
		before it */
		p: 1.5 * -a;
		q: 1.5 -;
		r: 1.5 *;
	}
</style>
