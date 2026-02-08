<script lang="ts">
	// Inter-arg block - BEFORE comma
	new A(a /* comment */, b);

	// Inter-arg block - AFTER comma
	new A(a, /* comment */ b);

	// Multiple inter-arg block
	new A(a /* c1 */, /* c2 */ b);

	// Leading comment in new
	new A(/* lead */ a, b);

	// Three args with inter-arg
	new A(a, /* comment */ b, c);

	// Comment before opening paren (prettier moves inside)
	new A(/* comment */ a, b);

	// Spread with comment
	new A(.../* comment */ arr);
</script>
