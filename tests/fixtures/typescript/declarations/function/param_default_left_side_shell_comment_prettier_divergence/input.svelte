<script lang="ts">
	// An own-line comment inside the grouping parens of a default value's LEFTMOST node: the
	// value's printer hoists the run ahead of its doc, and the `=`→value seam indents the value
	// one level under it — the form a line comment in the `=`'s own gap takes.
	function fn1(
		x = // c
			(a as any).b
	) {}
	function fn2(
		x = // c
			(a as any) ? b : c
	) {}
</script>
