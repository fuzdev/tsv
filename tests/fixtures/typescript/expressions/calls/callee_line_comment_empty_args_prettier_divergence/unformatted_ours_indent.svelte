<script lang="ts">
	// line comment between the callee and an empty argument list
	call // c1
	();

	// `new` with an empty argument list
	new Foo // c2
			();

	// after explicit type arguments
	call<A> // c3
();

	// two stacked line comments
	call // c4
// c5
	();

	// optional call, plain callee — the comment stays before `?.`
	call?. // c6
();

	// optional call, member callee — the comment stays before `?.`
	obj.m?. // c7
			();

	// member callee — the chain stays flat
	a.b // c8
			();

	// longer chain, same gap
	foo.bar().baz().qux // c9
();

	// a block comment does not end its line, so it stays inline
	call /* c10 */    ();

	// a block comment in an optional call stays before `?.`, glued to it
	call?. /* c11 */ ();

	obj.m?. /* c12 */ ();
</script>
