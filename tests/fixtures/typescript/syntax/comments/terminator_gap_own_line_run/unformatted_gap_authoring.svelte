<script lang="ts">
	// The other half of a terminator gap's run rule, at the emitter serving the
	// keywords whose operand may be parenthesized (`return` / `throw` /
	// `export default` / `export =`); the glue half is terminator_gap_glued_run.

	// An own-line LINE comment keeps its own line instead of trailing the `;` -
	// own-line-ness is asked of the source before the comment's kind.
	function fn1() {
		throw a
		// c1
		/* c2 */;
	}

	// A blank the author left above the run survives the terminator.
	function fn2() {
		return b

		/* c3 */;
	}
</script>
