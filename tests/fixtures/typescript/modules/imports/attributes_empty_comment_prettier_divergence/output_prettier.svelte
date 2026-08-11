<script>
	// source → with (block comment)
	import a from 'a' /* c1 */ with {};

	// with → brace
	import b from 'b' /* c2 */ with {};

	// inside empty braces
	import c from 'c' /* c3 */ with {};

	// after empty braces
	import d from 'd' with {}; /* c4 */

	// inside empty braces: a line comment can't be inlined, so it breaks them open
	import e from 'e' with {};
	// c5

	// inside empty braces: a run takes one line each, in source order
	import f from 'f' with {};
	// c6
	/* c7 */

	// two block comments break open too — the dangling separator is unconditional
	import g from 'g' with {};
	/* c8 */
	/* c9 */
</script>
