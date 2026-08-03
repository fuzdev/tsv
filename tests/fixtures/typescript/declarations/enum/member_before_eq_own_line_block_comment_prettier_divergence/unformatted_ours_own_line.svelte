<script lang="ts">
	enum E {
		// Single comment.
		A
		/* c */
		= 1,
		// A run keeps order, each comment distinct.
		B
		/* c1 */
		/* c2 */
		= 2
	}
</script>
