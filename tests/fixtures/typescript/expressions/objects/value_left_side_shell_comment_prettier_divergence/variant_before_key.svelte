<script lang="ts">
	// An own-line comment inside the grouping parens of a property value's LEFTMOST node: the
	// value's printer hoists the run ahead of its doc, and the `:`→value seam hangs the value
	// under the key, where the author put it.
	const o = {
		// c
		k: (a as any) ? b : c,
		// c
		m: (a as any).p
	};
</script>
