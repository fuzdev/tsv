<script>
	// a member expression is a DestructuringAssignmentTarget, so an ASSIGNMENT pattern
	// takes one; a binding pattern holds only identifiers and nested patterns
	[a1.b] = x;
	({ k: a2.b } = x);
	[...a3.b] = x;
	for ([a4.b] of xs);
	function f(a5 = ([b5.c] = x)) {}
</script>
