<script module lang="ts">
	export = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? ccccccccccccccccccccccccccccccccccccc ?? ddd;
</script>

<script lang="ts">
	// An `export =` value's chain takes a continuation indent - a TSExportAssignment
	// parent is in neither of prettier's exempt lists. The module script above is the
	// 100-char case and stays inline; this one is 101 chars, so the chain breaks.
	export = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccccccc ??
		ddd;
</script>
