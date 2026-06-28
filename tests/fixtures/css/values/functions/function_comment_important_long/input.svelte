<style>
	div {
		/* !important counts toward width; exactly 100 cols, stays inline. */
		prop: fn(/* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) !important;
		/* One col longer (101): the function wraps, ') !important' trails the paren. */
		prop: fn(
			/* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		) !important;
	}
</style>
