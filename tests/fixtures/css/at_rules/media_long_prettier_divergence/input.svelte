<style>
	/* @media wraps at 101 chars (we wrap at >100, Prettier never wraps) */

	/* 100 chars - stays inline (both agree) */
	@media (min-width: 1000px) and (max-width: 2000px) and (height > 300px) and (a: 100aaaaaaaaaaaa) {
		.a {
			color: red;
		}
	}

	/* 101 chars - we wrap, Prettier stays inline */
	@media (min-width: 1000px) and (max-width: 2000px) and (height > 300px) and
		(a: 100aaaaaaaaaaaaa) {
		.b {
			color: blue;
		}
	}

	/* 102 chars - we wrap, Prettier stays inline or preserves wrapping */
	@media (min-width: 1000px) and (max-width: 2000px) and (height > 300px) and
		(a: 100aaaaaaaaaaaaaa) {
		.c {
			color: green;
		}
	}

	/* 103 chars - we wrap, Prettier stays inline */
	@media (min-width: 1000px) and (max-width: 2000px) and (height > 300px) and
		(a: 100aaaaaaaaaaaaaaa) {
		.d {
			color: yellow;
		}
	}
</style>
