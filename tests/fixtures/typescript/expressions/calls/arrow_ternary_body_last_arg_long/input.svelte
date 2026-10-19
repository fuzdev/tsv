<script lang="ts">
	// a ternary-body arrow argument: the `=>` line is measured with the space a broken body leaves
	// call, the `=>` line at 99 chars - the body breaks
	fn('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		a ? b : c
	);
	// call, the `=>` line at 100 chars - the arguments break
	fn(
		'ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		(a) => (a ? b : c)
	);
	// member call, the `=>` line at 99 chars - the body breaks
	obj.fn('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		a ? b : c
	);
	// member call, the `=>` line at 100 chars - the arguments break
	obj.fn(
		'ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		(a) => (a ? b : c)
	);
	// `new`, the `=>` line at 99 chars - the body breaks
	new Fn('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		a ? b : c
	);
	// `new`, the `=>` line at 100 chars - the arguments break
	new Fn(
		'ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		(a) => (a ? b : c)
	);
	// member chain, the `=>` line at 99 chars - the body breaks
	a.b().c('ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		a ? b : c
	);
	// member chain, the `=>` line at 100 chars - the arguments break
	a.b().c(
		'tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		(a) => (a ? b : c)
	);
	// a lone argument, the `=>` line at 99 chars - the body breaks
	ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff((a) =>
		a ? bbbb : c
	);
	// a lone argument, the `=>` line at 100 chars - the argument breaks out whole
	fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff(
		(a) => (a ? bbbb : c)
	);
	// `new`, a lone argument, the `=>` line at 99 chars
	new FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF((a) =>
		a ? bbbb : c
	);
	// `new`, a lone argument, the `=>` line at 100 chars
	new FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF(
		(a) => (a ? bbbb : c)
	);
	// a member chain, a lone argument, the `=>` line at 99 chars
	x.y().ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff((a) =>
		a ? bbbb : c
	);
	// a member chain, a lone argument, the `=>` line at 100 chars
	x.y().fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff(
		(a) => (a ? bbbb : c)
	);
	// a lone argument broken out, too wide on its own line - the body breaks too
	fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff(
		(a) =>
			a
				? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
				: c
	);
	// a block comment ending its line after `=>`: the body breaks below it with no space
	// call, the `=>` line at 100 chars - the body breaks
	fn('ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		/* c */
		a ? b : c
	);
	// `new`, the `=>` line at 100 chars
	new Fn('ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		/* c */
		a ? b : c
	);
	// member chain, the `=>` line at 100 chars
	z.a()
		.b()
		.fn('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
			/* c */
			a ? b : c
		);
	// a doc comment, the `=>` line at 100 chars
	fn('ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		/** c */
		a ? b : c
	);
	// a lone argument, the `=>` line at 100 chars
	fn((attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
		/* c */
		a ? b : c
	);
	// a call body keeps the comment's line too
	fn('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		/* c */
		gg(a, b)
	);
</script>
