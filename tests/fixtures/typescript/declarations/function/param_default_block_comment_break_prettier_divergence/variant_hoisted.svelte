<script lang="ts">
	// A block comment glued to a parameter default `=` keeps its position; the value
	// reflows onto the `=` line. Prettier instead relocates the comment before the `=`.
	function fn(
		/* c */
		a = 1
	) {}
</script>
