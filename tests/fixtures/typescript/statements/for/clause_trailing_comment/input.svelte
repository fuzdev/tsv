<script>
	for (a /* c */; b; c) d();

	for (a; b /* c */; c) d();

	for (a; b; c /* c */) d();

	for (let a = 0 /* c */; b; c) d();
</script>
