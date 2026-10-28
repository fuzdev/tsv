<script lang="ts">
	// A comment in grouping parens the parser strips from around a member's object, in a member
	// chain, prints in front of that member only while that keeps the authored order. Where the
	// object or the member's own gap holds a comment too, it leads the chain instead, where the
	// spelling without the parens prints it.
	(/* c1 */ ((x) /* c2 */ ().h))[x]();
	(((/* c3 */ (/* c4 */ q).p)[x]).p)();
	(// c5
	x /* c6 */.b).g();
	(/* c7 */
	x /* c8 */.b).g();

	// Alone in the member's gap, it prints in front of the member.
	(/* c9 */ (x.y).p).t();

	// A comment in a computed lookup's brackets prints after the run either way.
	(/* c10 */ (x)[/* c11 */ i]).q();
</script>
