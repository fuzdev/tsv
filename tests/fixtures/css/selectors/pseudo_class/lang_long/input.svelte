<style>
	/* Fits at 100 columns — stays inline */
	span:lang(aaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbb, ccccccccccccccccccccccccccc) {
		color: red;
	}

	/* Exceeds 100 columns (101) — one range per line */
	span:lang(
		aaaaaaaaaaaaaaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbbbbbbbbbbbbbb,
		cccccccccccccccccccccccccccc
	) {
		color: blue;
	}
</style>
