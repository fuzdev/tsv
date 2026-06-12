<script lang="ts">
	// `using` as a plain identifier — member, index, and assignment targets,
	// not a resource declaration
	using[0] = a;
	using.b = c;
	using = d;

	// `await using` where `using` is a member expression is an await expression
	const fn = async () => {
		await using[0];
	};
</script>
