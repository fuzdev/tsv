<script lang="ts">
	// short binary with member chains - fits on one line
	const a = aa.bb.cc + dd.ee.ff;

	// paren + method call at 100 chars - fits on one line
	{
		(
			aaaa.bbbb_bbbb.cccc_cccc.dddd_dddd_dddd_dddddd + aaaa.bbbb_bbbb.cccc_cccc.eeee_eeee_eeee_eeeee
		).toString();
	}

	// paren + method call at 101 chars - break after operator
	{
		(
			aaaa.bbbb_bbbb.cccc_cccc.dddd_dddd_dddd_dddddd +
			aaaa.bbbb_bbbb.cccc_cccc.eeee_eeee_eeee_eeeeee
		).toString();
	}

	// assignment at 100 chars after = break - binary fits on one line
	const b =
		aaaa.bbbb_bbbb.cccc_cccc.dddd_dddd_dddd_dddddd + aaaa.bbbb_bbbb.cccc_cccc.eeee_eeee_eeee_eeeeee;

	// assignment at 101 chars after = break - break after operator
	const c =
		aaaa.bbbb_bbbb.cccc_cccc.dddd_dddd_dddd_ddddddd +
		aaaa.bbbb_bbbb.cccc_cccc.eeee_eeee_eeee_eeeeee;
</script>
