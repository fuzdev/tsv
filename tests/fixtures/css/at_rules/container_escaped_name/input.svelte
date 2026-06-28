<style>
	/* escaped @ in a container name keeps its backslash */
	@container \@named (min-width: 100px) {
		.a {
			color: red;
		}
	}

	/* escaped @ in a container condition value keeps its backslash */
	@container (\@foo: bar) {
		.b {
			color: blue;
		}
	}
</style>
