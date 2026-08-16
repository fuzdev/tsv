<script lang="ts">
	// Single comment.
	const { a }
	/* c */
	: T = x;

	// A run keeps order, each comment distinct.
	const [b]
	/* c1 */
	/* c2 */
	: U = y;
</script>
