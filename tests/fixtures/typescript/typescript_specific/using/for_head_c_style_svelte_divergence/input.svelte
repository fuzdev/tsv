<script lang="ts">
	// A C-style for head takes a using declaration like any lexical declaration
	for (using x = a; ;) {
		expr;
	}

	// Several declarators, a test and update, and a body-less loop
	for (using x = a, y = b; cond; i++) expr;

	// await using in an async function's C-style head
	async function f() {
		for (await using x = a; ;) {
			expr;
		}
	}
</script>
