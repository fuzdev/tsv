<script lang="ts">
	// A conditional or arrow as a unary operand needs its own parens. A leading
	// block comment glued inside those parens stays in the single pair where the
	// author wrote it — tsv does not add a second, comment-holder pair. prettier
	// relocates the comment out, before the operand's own parens.
	const a = !(/* c */ (cond ? b : c));
	const b = !(/* c */ (() => x));
</script>
