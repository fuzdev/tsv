<script lang="ts">
	// A type assertion as a unary operand needs its own parens; a leading block
	// comment glued inside them stays in that single pair. One pair suffices — a
	// redundant double-paren authoring collapses to it.
	const a = !(/* c */ x as T);
	const b = !(/* c */ x satisfies T);
</script>
