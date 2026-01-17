<script lang="ts">
	const x = $derived(
		cond && other
			? `<!DOCTYPE html>
<html>
<head>
<style></style>
</head>
<body>
${obj.content}
</body>
</html>`
			: alt,
	);
</script>
