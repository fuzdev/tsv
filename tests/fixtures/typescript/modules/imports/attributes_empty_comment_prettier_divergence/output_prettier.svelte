<script>
	// source → with (block comment)
	import a from 'a' /* c1 */ with {};

	// with → brace
	import b from 'b' /* c2 */ with {};

	// inside empty braces
	import c from 'c' /* c3 */ with {};

	// after empty braces
	import d from 'd' with {}; /* c4 */
</script>
