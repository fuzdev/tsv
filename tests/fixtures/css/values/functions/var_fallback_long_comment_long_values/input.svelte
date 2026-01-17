<style>
	div {
		/* 100 chars - comment + value fit on same line */
		prop: var(
			--a,
			/* comment that is quite long and extends past the print width limit */ cccccccccccccccccccccc
		);
		/* 101 chars - value breaks to separate line */
		prop: var(
			--a,
			/* comment that is quite long and extends past the print width limit */
			ccccccccccccccccccccccc
		);
	}
</style>
