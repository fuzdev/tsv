<script lang="ts">
	// Single comment.
	const o = {
		a
		/* c */
		: 1
	};

	// A run keeps order, each comment distinct.
	const p = {
		b
		/* c1 */
		/* c2 */
		: 2
	};
</script>
