<script>
	// line comment trailing `(` — single arg
	new Foo(a); // c1

	// line comment trailing `(` — multiple args
	new Foo(a, b); // c2

	// block comment trailing `(`, then own-line leading comment
	new Foo /* paren */(
		// lead
		a
	);
</script>
