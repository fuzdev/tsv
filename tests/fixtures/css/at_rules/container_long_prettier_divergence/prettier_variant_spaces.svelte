<style>
	/* @container wraps at 101 chars (we wrap at >100, Prettier never wraps) */

	/* 100 chars - stays inline (both agree) */
	@container (min-width: 1000px)   and   (max-width: 2000px)   and   (height > 300px)   and   (a: 100aaaaaaaa) {
		.a {
			color: red;
		}
	}

	/* 101 chars - we wrap, Prettier stays inline */
	@container (min-width: 1000px)   and
    (max-width: 2000px)   and   (height > 300px)   and   (a: 100aaaaaaaaa) {
		.b {
			color: blue;
		}
	}

	/* 102 chars - we wrap, Prettier stays inline */
	@container (min-width: 1000px)   and
    (max-width: 2000px)   and   (height > 300px)   and   (a: 100aaaaaaaaaa) {
		.c {
			color: green;
		}
	}

	/* 103 chars - we wrap, Prettier stays inline */
	@container (min-width: 1000px)   and
    (max-width: 2000px)   and   (height > 300px)   and   (a: 100aaaaaaaaaaa) {
		.d {
			color: yellow;
		}
	}
</style>
