<script lang="ts">
	const a = arr.map(
		([
			/* text1
text2 */ x,
			y
		]) => [y, x]
	);

	const b = arr.map(
		([
			/* text1
text2 */ x,
			y
		]) => x
	);

	const c = arr.map(([/* text */ x, y]) => [y, x]);
</script>
