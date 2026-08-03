<script lang="ts">
	class C {
		// Single comment.
		a
		/* c */
		= 1;

		// A run keeps order, each comment distinct.
		b
		/* c1 */
		/* c2 */
		= 2;
	}
</script>
