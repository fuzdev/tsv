<script>
	// Multi-arg head, arrow callback with call body - 100 chars (stays inline)
	f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 1).g((x) => c());

	// Multi-arg head, arrow callback with call body - 101 chars (breaks)
	f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 1).g((x) =>
		c()
	);

	// Multi-arg head, arrow callback with ternary body - 100 chars (stays inline)
	f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 1).g((x) => (x ? a() : b()));

	// Multi-arg head, arrow callback with ternary body - 101 chars (breaks, parens stripped)
	f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 1).g((x) =>
		x ? a() : b()
	);

	// Multi-arg head, arrow callback with array body
	f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 1).g((x) => [
		x
	]);
</script>
