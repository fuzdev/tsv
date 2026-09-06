<script>
	let a, b, c, d, e, f, g1, g2, h1, h2, i;

	// An indentable multi-line block comment glued to the value hangs the value under the
	// `=` whatever the value's own layout would be: a regex-rooted call, a literal-based
	// member chain and a member call with an arrow argument hang exactly as a plain call does.
	a =
		/**
		 * c
		 */ /re/.exec(x);
	b =
		/**
		 * c
		 */ 'aaa'.length;
	c =
		/**
		 * c
		 */ obj.p.q.filter((v) => v);

	// A preserved multi-line block comment glued to the value keeps the value on the `=`
	// line: the comment's own interior already ends that line, so a break at `=` has
	// nothing left to decide.
	d = /* line1
	line2 */ /re/.exec(x);
	e = /* line1
	line2 */ 'aaa'.length;
	f = /* line1
	line2 */ obj.p.q.filter((v) => v);

	// The same two rules under a destructuring target
	({ g1, g2 } =
		/**
		 * c
		 */ /re/.exec(x));
	({ h1, h2 } = /* line1
	line2 */ /re/.exec(x));

	// A chain holding a line comment of its own hangs too, and still breaks at the comment
	i =
		/**
		 * c
		 */ obj // x
			.p()
			.q();
</script>
