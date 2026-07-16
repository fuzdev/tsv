<script lang="ts">
	// An own-line multiline block comment in an indexed access's `[`→index gap hangs
	// the index on its own line, the comment kept inside the brackets where the author
	// wrote it — inlining it would reflow the author's break. A GLUED multiline block
	// collapses inline instead (keyword_value_glued_multiline_block_comment), as does a
	// single-line block in any position (indexed_access_own_line_block_comment).
	type X =
		A[/* c
		d */
		K];
</script>
