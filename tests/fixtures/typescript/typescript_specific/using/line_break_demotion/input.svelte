<script lang="ts">
	// A line break between `using` and the binding demotes `using` to a plain
	// identifier — `using [no LineTerminator here] BindingIdentifier` — so this
	// is an expression statement followed by an assignment, not a declaration
	using;
	a = 1;

	// The second gap of `await using` carries the same restriction: a break
	// before the binding leaves an `await using` expression behind
	const fn = async () => {
		await using;
		b = 1;
	};
</script>
