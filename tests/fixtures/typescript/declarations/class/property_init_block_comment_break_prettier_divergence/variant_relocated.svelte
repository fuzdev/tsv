<script lang="ts">
	// A block comment glued to `=` keeps its position; the value reflows onto the
	// `=` line. Prettier instead relocates the comment before the `=`.
	class C {
		p /* c */ = 1;
	}
</script>
