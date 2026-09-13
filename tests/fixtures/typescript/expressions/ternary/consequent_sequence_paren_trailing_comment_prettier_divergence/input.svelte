<script lang="ts">
	// A block comment inside the grouping shell the parser erased from a ternary
	// CONSEQUENT sequence's last operand. The consequent's own scan stops where the `:`
	// gap's emitter starts, so the region behind the operands is the sequence's own tail
	// and the comment prints after the pair.
	const a = cond ? (x, y) /* t1 */ : z;

	// A run of two answers the same way.
	const b = cond ? (x, y) /* t2 */ /* t3 */ : z;
</script>
