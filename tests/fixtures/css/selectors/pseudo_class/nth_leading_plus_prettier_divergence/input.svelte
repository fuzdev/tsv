<style>
	/* leading + on a coefficient: tsv keeps it glued to the An+B term */
	:foo(+2n + 1) {
		color: red;
	}

	/* leading + on a plain integer B */
	:is(+3) {
		color: blue;
	}

	/* leading + on a bare n */
	:not(+n) {
		color: green;
	}

	/* leading + via the dedicated :nth-child An+B path */
	:nth-child(+2n + 1) {
		color: yellow;
	}
</style>
