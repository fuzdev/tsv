<script lang="ts">
	// `using` heading a cast statement keeps its pair: bare, `using as` / `using satisfies`
	// opens a `using` declaration binding a name `as` / `satisfies` (tsc; acorn at ES2026)
	(using) as T;
	(using) satisfies T;
	(using) as T as U;

	// every statement position holding an expression statement
	if (a) (using) as T;
	L: (using) satisfies T;

	// `await using` is its own operand, so the pair wraps the pair of words
	const fn1 = async () => {
		(await using) as T;
		(await using) satisfies T;
	};
</script>
