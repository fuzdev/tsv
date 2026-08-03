<script lang="ts">
	// Single comment.
	interface A {
		[
			key /* c */
			: string
		]: number;
	}

	// A run keeps order, each comment distinct.
	interface B {
		[
			key /* c1 */
			/* c2 */
			: string
		]: number;
	}
</script>
