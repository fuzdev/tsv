<script lang="ts">
	interface I {
		a /* c1 */;
		readonly b /* c2 */;
		c: number;
	}

	type T = { a /* c3 */; d: number };
</script>
