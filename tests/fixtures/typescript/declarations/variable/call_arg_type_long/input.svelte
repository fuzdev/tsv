<script lang="ts">
	// Call args break when line exceeds 100 (101 visual chars)
	const a: Array<{ b: string; c: string; d: typeof eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }> = fn(
		[]
	);

	// Stays inline at exactly 100 visual chars
	const f: Array<{ b: string; c: string; d: typeof eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }> = fn([]);
</script>
