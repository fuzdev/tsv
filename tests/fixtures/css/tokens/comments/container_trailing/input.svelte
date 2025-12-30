<style>
	/* single condition */
	@container (min-width: 100px) /* trailing */ {
		div {
			color: red;
		}
	}

	/* multi-condition */
	@container (min-width: 100px) and (max-width: 200px) /* trailing */ {
		div {
			color: blue;
		}
	}
</style>
