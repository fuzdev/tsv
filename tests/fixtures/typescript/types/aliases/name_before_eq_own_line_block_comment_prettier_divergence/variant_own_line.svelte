<script lang="ts">
	// Single comment.
	type A =
		/* c */
		number;

	// A run keeps order, each comment distinct.
	type B =
		/* c1 */
		/* c2 */
		string;
</script>
