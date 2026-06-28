<script>
	// source → with: both keep the block comment in place (dual-stable)
	import a from 'a' /* c1 */ with {};

	// with → brace: tsv keeps it in place; prettier relocates before `with`
	import b from 'b' with /* c2 */ {};

	// inside empty braces: tsv keeps it in place; prettier relocates before `with`
	import c from 'c' with {/* c3 */};

	// after empty braces: both trail the comment past `;`
	import d from 'd' with {} /* c4 */;
</script>
