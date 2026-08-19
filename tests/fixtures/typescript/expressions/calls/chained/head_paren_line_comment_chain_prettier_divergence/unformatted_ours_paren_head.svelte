<script lang="ts">
	// A `//` the author wrote inside stripped grouping parens at a chain's HEAD.
	// Prettier binds it to the base node, so the break it forces lands inside the
	// chain's first group and expands the whole chain; tsv prints it above the chain
	// and keeps the layout the same document gets with the parens left out.

	(// c1
	a).b.c(x).d(y);

	((// c2
	a).b).c(x).d(y);
</script>
