<script>
	// source → with (block comment)
	import a from 'a' /* c1 */ with {};

	// with → brace
	import b from 'b' with /* c2 */ {};

	// inside empty braces
	import c from 'c' with {/* c3 */};

	// after empty braces
	import d from 'd' with {}; /* c4 */
</script>
