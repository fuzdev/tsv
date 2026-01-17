<script lang="ts">
	async function fn() {
		/* negation */
		if ( ! ( await a ) ) {
		}

		/* typeof */
		const t = typeof ( await b );
		/* void */
		void ( await c );
	}
</script>
