<style>
	/* a non-last selector is measured through its `,` alone: 100 chars stays inline */
	@custom-selector :--a
		.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:is(.xx, .yy),
		.b;
	/* at 101 its arguments break */
	@custom-selector :--a
		.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:is(
			.xx,
			.yy
		),
		.b;
	/* the last selector is measured through the `;`: 100 chars stays inline */
	@custom-selector :--a
		.b,
		.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:is(.xx, .yy);
	/* 101 chars breaks */
	@custom-selector :--a
		.b,
		.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:is(
			.xx,
			.yy
		);
	/* a pseudo's arguments are measured through the next pseudo's `(` alone: 100 stays inline */
	@custom-selector :--a
		:is(.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, .yy):not(
			.zz,
			.ww
		),
		.b;
	/* 101 chars - its arguments break */
	@custom-selector :--a
		:is(
			.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			.yy
		):not(.zz, .ww),
		.b;
</style>
