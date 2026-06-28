<style>
	div {
		/* !important counts toward width: with it this is exactly 100 cols, stays inline. */
		prop: fn(aaaaaaaaa, bbbbbbbbb, ccccccccc, ddddddddd, eeeeeeeee, fffffffff, gggggggg) !important;
		/* One col longer (101): the function wraps one arg per line, ') !important' trails. */
		prop: fn(
			aaaaaaaaa,
			bbbbbbbbb,
			ccccccccc,
			ddddddddd,
			eeeeeeeee,
			fffffffff,
			ggggggggg
		) !important;
	}
</style>
