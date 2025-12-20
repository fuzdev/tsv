<style>
	div {
		/* drop-shadow with many parameters - wraps when exceeding print width */
		filter: drop-shadow(
			0000000000000000px 00000000000000000px 00000000000000000px rgba(0, 0, 0, 0)
		);
		/* drop-shadow with nested calc - wraps when exceeding print width */
		filter: drop-shadow(
			calc(0px + 0px + 0px + 0px) calc(0000000px + 00000000px) 0 rgba(0, 0, 0, 0)
		);
		/* url filter with long path - single-arg function, doesn't wrap */
		filter: url('00000000000000000000000000000000000000000000000000000000000000000000000000000000000');
	}
</style>
