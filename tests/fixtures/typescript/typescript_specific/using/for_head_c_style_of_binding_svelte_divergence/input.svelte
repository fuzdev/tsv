<script lang="ts">
	// A using declaration in a C-style head may bind `of`
	for (using of = a; ;) {
		expr;
	}

	// with a type annotation, a second declarator, a test and update
	for (using of: T = a, b = c; cond; i++) expr;

	// `using of` followed by an expression is a for-of over the identifier `using`
	for (using of items) {
		expr;
	}
</script>
