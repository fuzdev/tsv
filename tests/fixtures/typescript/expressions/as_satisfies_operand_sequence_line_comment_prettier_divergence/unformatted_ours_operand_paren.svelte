<script lang="ts">
	// A SEQUENCE operand rides the retained ASI shell BARE — that shell IS the sequence's
	// required pair — so both of the shell's gaps are the shell's to emit, the trailing one
	// included, and the operands stay on the line the layout gives them.
	const a = (
		// c1
		x, (y // c2
		)
	) as A;

	// The trailing gap reaches inside the shell the parser erased from the LAST operand,
	// which closes inside the pair; a `satisfies` operand answers the same.
	const b = (
		// c3
		x, (y /* c4 */)
	) satisfies B;
</script>
