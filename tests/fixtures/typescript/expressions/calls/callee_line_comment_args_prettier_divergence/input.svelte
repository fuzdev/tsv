<script lang="ts">
	// line comment between the callee and a NON-empty argument list
	call // c1
		(a);

	// `new` with arguments
	new Foo // c2
		(b);

	// after explicit type arguments
	call<A> // c3
		(c);

	// two stacked line comments
	call // c4
		// c5
		(d);

	// member callee — the chain stays flat
	a.b // c6
		(e);

	// longer chain, same gap
	foo.bar().baz().qux // c7
		(f);

	// a dynamic import answers the gap the same way
	const g = import // c8
		('./g');

	// a block comment does not end its line, so it leads the first argument inline
	call(/* c9 */ h);
</script>
