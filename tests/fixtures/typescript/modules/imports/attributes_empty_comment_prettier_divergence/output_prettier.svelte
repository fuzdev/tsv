<script>
	// source → with: both keep the block comment in place (dual-stable)
	import a from 'a' /* c1 */ with {};

	// with → brace: tsv keeps it in place; prettier relocates before `with`
	import b from 'b' /* c2 */ with {};

	// inside empty braces: tsv keeps it in place; prettier relocates before `with`
	import c from 'c' /* c3 */ with {};

	// after empty braces: tsv keeps it after `}`; prettier relocates before `with`
	import d from 'd' /* c4 */ with {};
</script>
