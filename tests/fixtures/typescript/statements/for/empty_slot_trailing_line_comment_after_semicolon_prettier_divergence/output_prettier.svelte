<script>
	// A `//` on the init `;`'s line before an empty test slot trails that `;`
	for (
		;
		;
		// c1
		i++
	) {}

	// The update slot's twin: a `//` on the test `;`'s line trails it
	for (
		a; // c2
		;
	) {}

	// A run: the first `//` trails the `;`, the second keeps the slot's own line
	for (
		a; // c3
		;
		// c4
		i++
	) {}

	// A block after the trailing `//` keeps the slot's own line too
	for (
		a; // c5
		;
		/* c6 */
		i++
	) {}

	// The update slot's run
	for (
		a;
		b; // c7
	)
	// c8
	{}

	// A `//` the test hangs on the `;` keeps it: the slot's own comment takes the next line
	for (
		a;
		b; // c9
	)
	// c10
	{}

	// So it does after an own-line comment the clause before the `;` defers past it
	for (
		a;
		b; // c11
	)
	// c12
	// c13
	{}

	// The init's `//` on the `;`, with an own-line comment deferred past it
	for (
		a; // c14
		;
		// c15
		// c16
		i++
	) {}
</script>
