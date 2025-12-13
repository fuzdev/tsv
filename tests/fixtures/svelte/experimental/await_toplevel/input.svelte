<script>
	const response = await fetch('/api/data');
	const data = await response.json();

	const [a, b] = await Promise.all([fetchA(), fetchB()]);
</script>

<p>{data.message}</p>
<p>{a} and {b}</p>
