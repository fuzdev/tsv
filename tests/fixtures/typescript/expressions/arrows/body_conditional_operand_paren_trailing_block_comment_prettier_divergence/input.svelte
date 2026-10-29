<script>
	// A block written last inside the pair an arrow's conditional body keeps around a `??`
	// operand stays inside that pair, where the author wrote it.
	x = () => (c ? d : (a ?? b /* c */));
</script>
