<script lang="ts">
	// A multi-line block leading a parenthesized base stays inside the pair, on the `(`
	// line. A ternary base keeps its own layout around it: the `)` drops to its own line
	// when the ternary breaks, and the ternary stays flat when it fits.
	(/* c1
	 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb : cccccccccccccccccccccc).d;
	(/* c2
	 */ a ? b : c).d;

	// A base that would hang inside its parens hugs them instead, binary and cast bases
	// included: the block keeps the `(` line and the operand breaks one indent in.
	(/* c3
	 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + ccccccccccccc).d;
	(/* c4
	 */ a + b).d;
	(/* c5
	 */ fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) as Bbbbbbbbbbbbbbbbb).d;
	(/* c6
	 */ a as B).d;
</script>
