<script>
	// A superclass expression's chain takes a continuation indent - a ClassDeclaration
	// parent is in neither of prettier's exempt lists
	class A extends (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccc ??
		ddd) {}

	// 100 chars - stays inline
	class B extends (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? cccccccccccccccccccccccccc ?? ddd) {}

	// 101 chars - the chain breaks
	class C extends (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		ccccccccccccccccccccccccccc ??
		ddd) {}
</script>
