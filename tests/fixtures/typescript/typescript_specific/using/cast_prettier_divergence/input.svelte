<script lang="ts">
	// `using`/`await using` followed by `as`/`satisfies` are cast expressions
	// of the identifier `using`, not declarations of a binding named
	// `as`/`satisfies` (see prettier_rejects.txt)
	using as T;
	using satisfies T;
	const fn1 = async () => {
		(await using) as T;
		(await using) satisfies T;
	};
</script>
