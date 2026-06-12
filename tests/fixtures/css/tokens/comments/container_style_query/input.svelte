<style>
	/* trailing comment after style query */
	@container style(--theme: dark) /* trailing */ {
		div {
			color: red;
		}
	}

	/* named container with style query */
	@container sidebar style(--highlight: true) /* trailing */ {
		div {
			color: blue;
		}
	}
</style>
