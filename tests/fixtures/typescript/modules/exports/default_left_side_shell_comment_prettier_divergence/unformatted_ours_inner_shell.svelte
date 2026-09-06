<script lang="ts">
	// An own-line comment inside the grouping parens of the exported value's LEFTMOST node:
	// the value's printer hoists the run ahead of its doc, and the keyword→value seam hangs the
	// value under `export default` — the form a comment in the keyword's own gap takes.
	export default (
		// c
		a as any
	).b;
</script>
