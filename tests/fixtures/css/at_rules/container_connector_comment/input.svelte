<style>
	/* a comment between conditions stays in place */
	@container (width > 0px) /* note */ and (height > 0px) {
		.a {
			color: red;
		}
	}

	/* a comment whose text contains the `and` connector word stays put */
	@container (width > 0px) /* a and b */ and (height > 0px) {
		.b {
			color: blue;
		}
	}
</style>
