<script lang="ts">
	interface I {
		a: string
		b(): number
		async: boolean
	}

	type T = { a: string; b: number };
</script>
