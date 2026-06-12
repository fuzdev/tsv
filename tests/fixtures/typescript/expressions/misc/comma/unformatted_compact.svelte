<script lang="ts">
	// Basic sequence expressions
	const x = (a,b,c);
	const y = (1+2,3*4,5);

	// Sequence as function argument (requires extra parens)
	foo((a,b));
	bar((x,y,z));

	// Standalone expression statement
	(a,b,c);

	// Nested sequence expressions
	const nested = ((a,b),(c,d));

	// Sequence in computed property access
	const computed = obj[(a,b)];

	// Sequence with complex expressions
	const complex = (a+b,c*d,e-f);

	// Sequence with ternary
	const withTernary = (a?b:c,d);

	// Sequence with function calls
	const withCalls = (fn1(),fn2(),fn3());
</script>
