<script>
	// An indentable multi-line block comment glued to the value hangs the value under the
	// `=` whatever the value's own layout would be: a regex-rooted call, a literal-based
	// member chain and a member call with an arrow argument hang exactly as a plain call does.
	const a =
		/**
		 * c
		 */ /re/.exec(x);
	const b =
		/**
		 * c
		 */ 'aaa'.length;
	const c =
		/**
		 * c
		 */ obj.p.q.filter((v) => v);

	// A preserved multi-line block comment glued to the value keeps the value on the `=`
	// line: the comment's own interior already ends that line, so a break at `=` has
	// nothing left to decide.
	const d = /* line1
	line2 */ /re/.exec(x);
	const e = /* line1
	line2 */ 'aaa'.length;
	const f = /* line1
	line2 */ obj.p.q.filter((v) => v);

	// The same two rules under a destructuring binding
	const { g1, g2 } =
		/**
		 * c
		 */ /re/.exec(x);
	const { h1, h2 } = /* line1
	line2 */ /re/.exec(x);

	// A chain holding a line comment of its own hangs too, and still breaks at the comment
	const i =
		/**
		 * c
		 */ obj // x
			.p()
			.q();
</script>
