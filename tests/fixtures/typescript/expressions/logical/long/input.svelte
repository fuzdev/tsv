<script>
	/* 100 chars - stays inline */
	const a = cond_aaaaaaaaaaaaaaa || cond_bbbbbbbbbbbbbbbbb || cond_cccccccccccccccccccccccccccccccc;

	/* 101 chars - breaks after = */
	const b =
		cond_aaaaaaaaaaaaaaa || cond_bbbbbbbbbbbbbbbbb || cond_ccccccccccccccccccccccccccccccccc;

	/* Inside Boolean() - wraps arguments */
	const foo = Boolean(
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	);

	/* Inside !!() - wraps arguments */
	const bar = !!(
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	);
</script>
