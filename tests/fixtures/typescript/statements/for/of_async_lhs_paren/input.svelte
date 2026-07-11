<script lang="ts">
	// `async` as the for-of LHS keeps parens — bare `for (async of [])` is a syntax error
	for ((async) of []);
	// ordinary identifier LHS — redundant parens stripped
	for (foo of []);
</script>
