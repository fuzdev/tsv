<script>
	/* short - stays inline */
	const a = b || /* comment */ c || d;

	/* condition 100 chars - stays inline */
	if (aaaa.bbbb === 'cccc' && /* comment */ dddd.eeee === 'ffff' && gggg.hhhhhhhhhhhhhhhhhhhhhhhh) {
		a;
	}

	/* condition 101 chars - breaks, comment leads RHS */
	if (
		aaaa.bbbb === 'cccc' &&
		/* comment */ dddd.eeee === 'ffff' &&
		gggg.hhhhhhhhhhhhhhhhhhhhhhhhh
	) {
		a;
	}

	/* assignment - binary chain breaks after =, comment leads RHS */
	const d =
		aaaa_aaaa.bbbb_bbbb === 'cccccccc' ||
		/* comment */ dddd_dddd.eeee_eeee === 'ffffffff' ||
		gggg_gggg.hhhh_hhhh;
</script>
