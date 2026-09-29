<script>
	// a `//` inside a pair the printer strips defers to the end of its line: it must
	// flush inside the list the operand sits in, never past its `)`
	function f1(v = x + (y * z // c1
	), w) {}
	f(x + (y * z // c2
	), 1);
	f(1, x + (y * z // c3
	));
	f(a instanceof (b // c4
	), 1);
	f(x - ((y - z) // c5
	), 1);
	function f2({ v = x + (y * z // c6
	) }) {}

	// a sequence, whose own pair floats the run out past its `)`
	f(a + (b, c // c7
	), 1);
	f(a ? b : (c, d // c8
	), 1);

	// a statement ends the line anyway: the run trails it, as in both formatters
	const r = x + (y * z // c9
	);
</script>
