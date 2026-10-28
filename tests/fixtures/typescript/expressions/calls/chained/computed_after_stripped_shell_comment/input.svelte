<script lang="ts">
	// A computed lookup whose object holds a comment, inside grouping parens the parser strips
	// from around the lookup in a chain with a link after them. The comment prints once, where
	// the object prints it, never a second time inside the lookup's brackets.
	[] /* c1 */()[x].a();
	[] /* c2 */()[x].a;
	[] /* c3 */()[x].a();
	[]
		.a(
			x // c4
		)
		[x].r();
	[] // c5
		.c()
		[x].m();
	x = [x /* c6 */][x]?.u();
	const v = [] /* c7 */()[x].a();
	function f() {
		return [] /* c8 */()[x].a();
	}
	g([] /* c9 */()[x].a());

	// The `[` right behind the comment, and the same lookups with no shell (the controls).
	[] /* c10 */[x]
		.a();
	[] /* c11 */()[x].a();
</script>

<p>{[] /* c12 */()[x].a()}</p>
