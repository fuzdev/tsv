<script lang="ts">
	// A block comment glued to `=` keeps its position; the value reflows onto the
	// `=` line, uniform with every other initializer.
	enum E {
		A = /* c */
			1,
		B = 2
	}
</script>
