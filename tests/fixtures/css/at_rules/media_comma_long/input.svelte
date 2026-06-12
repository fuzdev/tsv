<style>
	/* short comma-list - stays inline */
	@media screen, print {
		a {
			x: 1;
		}
	}
	/* 100 chars - stays inline (both formatters) */
	@media only screen and (min-width: 1000px), only screen and (orientation: aaaaaaaaaaaaaaaaaaaaa) {
		a {
			x: 1;
		}
	}
	/* 101 chars - breaks at each comma (one query per line) */
	@media only screen and (min-width: 1000px),
		only screen and (orientation: aaaaaaaaaaaaaaaaaaaaaa) {
		a {
			x: 1;
		}
	}
</style>
