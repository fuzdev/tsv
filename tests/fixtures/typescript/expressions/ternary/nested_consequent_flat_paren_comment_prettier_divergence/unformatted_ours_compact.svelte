<script>
	// A comment in the `?`→consequent gap ahead of a nested conditional that fits flat: the
	// gap's run prints ahead of the pair the flat layout mints, and a block glued to the
	// nested test prints inside it — whichever side the author wrote. prettier moves the run
	// inside the pair (`? (/* c */ aaa ? bbb : ccc)`) — see README.
	const x1 = a?/* c */(aaa?bbb:ccc):c;
	const x2 = a?(/* c */aaa?bbb:ccc):c;

	// a run of two
	const x3 = a?/* c *//* d */(aaa?bbb:ccc):c;

	// controls: at a clarity pair both formatters keep the run outside; the alternate
	// mints no pair
	const x4 = a?/* c */(b??c):d;
	const x5 = a?b:/* c */aaa?bbb:ccc;
</script>
