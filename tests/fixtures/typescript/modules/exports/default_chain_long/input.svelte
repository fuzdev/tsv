<script module>
	export default bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? ccccccccccccccccccccccccccccccc ?? ddd;
</script>

<script>
	// An exported default expression's chain takes a continuation indent - an
	// ExportDefaultDeclaration parent is in neither of prettier's exempt lists. The
	// module script above is the 100-char case and stays inline; this one is 101
	// chars, so the chain breaks.
	export default bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccc ??
		ddd;
</script>
