<script lang="ts">
	// A function type has no comparison reading to fall back to — `(…) =>` is no relational
	// operand — so a follower that ends the list for any other head is a syntax error here.
	const a = f<(b: T) => U> + 1;
</script>
