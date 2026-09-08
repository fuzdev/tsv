<style>
	div {
		/* the leading run and the first word fill the first line to exactly 100: they stay together */
		background: /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
		/* 101 columns: the value breaks after the run onto a continuation line */
		background: /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
		/* the same boundary at a long property */
		prop000000000000000000000000000: /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
		prop000000000000000000000000000: /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
		/* a multi-comment run breaks at its last boundary, between the run and the word */
		background: /* c1 */ /* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
	}
</style>
