<script lang="ts">
	// A trailing comment on the last operand of a parenthesized sequence in a value
	// position (return / variable init / assignment RHS) stays inside the parens.
	function f() {
		return (a, b /* c */);
	}
	function g() {
		return (aaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbb /* trailing block */);
	}
	function h() {
		// a line comment forces the sequence to break, the comment trailing the last
		// operand inside the parens
		return (
			aa,
			bb // trailing line
		);
	}
	const x = (a, b /* c */);
	y = (a, b /* c */);
</script>
