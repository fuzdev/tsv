<script lang="ts">
	// Single comment: stays in its authored gap, before the `:`.
	interface I {
		[x]
		/* c */
		: number;
	}

	// A run keeps order, each comment distinct.
	type T = {
		[y] /* c1 */ /* c2 */: string;
	};
</script>
