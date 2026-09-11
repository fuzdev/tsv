<script lang="ts">
	// A block comment glued to a parameter default `=` keeps its position; the value
	// reflows onto the `=` line. Prettier instead relocates the comment before the `=`.
	function fn(a = /* c */ 1) {}

	// a multi-line comment glued to the value below the block: same, and the value stays flat
	function fn2(
		a2 = /* c1 */ /* c2
		 */ b2 ? c2 : d2
	) {}
</script>
