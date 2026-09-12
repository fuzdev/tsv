<script lang="ts">
	// the run splits at its last break: what ends its line stays above the `|`, only the
	// tail glued to the union's head leads the first member
	type A = /* c1 */
	/* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	// an own-line comment in the prefix keeps its own line
	type B = /* c1 */
	/* c0 */
	/* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	// the annotation `:` answers like the alias `=`
	let c: /* c1 */
	/* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	// the tail is a glued RUN, not just its last comment
	type D = /* c1 */
	/* c2 */ /* c3 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	// a glued pair in the prefix stays on one line above the `|`
	type E = /* c1 */ /* c2 */
	/* c3 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	// an authored leading `|` splits the same way
	type F = /* c1 */
	/* c2 */ | aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	// control: a run glued all the way to the head leads the first member whole
	type G = /* c1 */ /* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	// control: with no glued tail the whole run stays above the `|`
	type H = /* c1 */
	/* c2 */
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	// control: a MULTI-LINE block binds to the union even glued to its head
	type I = /* c1 */
	/* c2
	d */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	// control: a run authored INSIDE the union, after its `|`, is the member's whole
	type J =
		| /* c1 */
		/* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	// control: a union that fits collapses the whole run onto the value's line
	type K = /* c1 */
	/* c2 */ a | b;

	// an author blank inside the prefix survives, above the `|`
	type L = /* c1 */

	/* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	// the function-type `=>` and a type argument's `<` share the union's answer
	type M = () => /* c1 */
	/* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc;

	type N = Ref</* c1 */
	/* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccc>;
</script>
