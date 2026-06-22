<script lang="ts">
	// Arrow with body that already has hardlines (nested callback with block body)
	const x = arr.map((item) =>
		aaaaaaa(item, (bbbbbb) => {
			console.log(bbbbbb);
		})
	);
</script>
