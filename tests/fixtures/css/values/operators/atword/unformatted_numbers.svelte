<style>
	div {
		/* an `@`-word is a member of its own: the ordinary member gap stands on either
		side of it, whether or not the author wrote one, and its neighbours normalize */
		a: a @b;
		b: @a @b;
		c: 1.50 @a;
		d: #FFF @a;
		e: "x" @a;
		f: @a "x";
		/* a group after an `@`-word is a second member, not an argument list — `@a` is no
		function name — so the ordinary gap stands there too, inside a function as well */
		g: @a (2.50);
		h: a @b (2.50);
		i: f(@a (2.50), 1.50);
		/* an empty `@` is a member too, and keeps its authored glue to the group after it */
		j: a @(2.50);
		k: @(2.50);
		/* an operator beside an `@`-word takes the gaps it takes beside any word */
		l: @a / 1.50;
		m: 1.50 / @a (2.50);
		/* the word's own extent: it ends at whitespace, a quote, a comma, a paren, a
		brace, a `/` or another `@`, so `*`, `+`, `-`, `:`, `#`, `.` and a digit are its
		content */
		n: @a*1.50;
		o: @a+1.50;
		p: @a-1.50;
		q: @a1.50;
		r: @a#FFF;
		s: f(@a:1.50);
		/* a `-` straight before an `@`-word is an operator glued to it, at the head, after
		another operator and inside a function alike */
		t: -@a;
		u: 1.5 * -@a;
		v: f(-@a);
	}
</style>
