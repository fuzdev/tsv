<style>
	/* 100 chars - stays inline (both agree) */
	@import url('a.css') supports((display: grid) and (gap: 1px) and (x: yyyyyyyyyyyyyyyyyyyyyyyyyy));

	/* 101 chars - we break the condition, Prettier stays inline */
	@import url('a.css') supports(
		(display: grid) and (gap: 1px) and (x: yyyyyyyyyyyyyyyyyyyyyyyyyyy)
	);

	/* 102 chars - both break, at different points */
	@import url('a.css') supports(
		(display: grid) and (gap: 1px) and (x: yyyyyyyyyyyyyyyyyyyyyyyyyyyy)
	);
</style>
