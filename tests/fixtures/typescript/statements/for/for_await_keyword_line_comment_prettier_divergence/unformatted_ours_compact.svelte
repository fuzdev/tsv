<script>
	// `for`→`await` gap: the line comment trails `for`, and `await (…)` drops to
	// the next line so the `//` can't swallow it.
	for // c
	await(const a of x){b();}

	// `await`→`(` gap: the line comment trails `await`, and `(…)` drops to the
	// next line.
	for await // c
	(const c of y){d();}
</script>
