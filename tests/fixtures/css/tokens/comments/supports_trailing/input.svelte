<style>
	/* single condition */
	@supports (display: grid) /* trailing */ {
		div {
			color: red;
		}
	}

	/* multi-condition */
	@supports (display: grid) and (flex: 1) /* trailing */ {
		div {
			color: blue;
		}
	}
</style>
