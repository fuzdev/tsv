<script lang="ts">
	// Consecutive line comments between `else` and its alternate: the first trails
	// `else`, each subsequent one keeps its own line (must not merge).
	if (a) {
		fn1();
	} else // c1
	// c2
	if (b) {
		fn2();
	}
</script>
