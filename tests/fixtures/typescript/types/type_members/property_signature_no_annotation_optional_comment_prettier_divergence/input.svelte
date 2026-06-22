<script lang="ts">
	interface I {
		a? /* c */;
		b: number;
	}

	type T = { a? /* c */; e: number };
</script>
