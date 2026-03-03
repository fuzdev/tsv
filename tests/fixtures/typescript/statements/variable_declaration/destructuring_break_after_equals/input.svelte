<script lang="ts">
	// Object destructuring + await call: pattern + call head exceeds 100, break at = (101 visual)
	const {aaaa_bbbb_cccc_dddd, eeee_ffff_gggg_hhhh, iiii_jjjj_kkkk_llll, mmmm_nnnn_ooooo} =
		await fn1(xxxx, yyyy, zzzz);

	// Object destructuring + call (no await): break at = (101 visual)
	const {pppp_qqqq_rrrr_ssss, tttt_uuuu_vvvv_wwww, xxxx_yyyy_zzzz_aaab, bbbc_cccd_dddee} =
		fn1_fn2_fn3(xx1, yy1, zz1);

	// Array destructuring + await call: break at = (101 visual)
	const [eeef_fffg_gggh_hhhj, iiik_jjjl_kkkm_llln, mmmo_nnnp_oooq_pppr, qqrs_rrst_sstuu] =
		await fn2(xx2, yy2, zz2);

	// Object destructuring + await call at exactly 100: args expand, no break at = (100 visual)
	const {aaa1_bbb1_ccc1_ddd1, eee1_fff1_ggg1_hhh1, iii1_jjj1_kkk1_lll1, mmm1_nnn1_ooo1} = await fn4(
		xx4,
		yy4,
		zz4,
	);

	// Identifier RHS: break at = (101 visual)
	const {aaa2_bbb2_ccc2_ddd2, eee2_fff2_ggg2_hhh2, iii2_jjj2_kkk2_lll2, mmm2_nnn2_ooo2} =
		identName2;

	// New expression: break at = (101 visual)
	const {aaa3_bbb3_ccc3_ddd3, eee3_fff3_ggg3_hhh3, iii3_jjj3_kkk3_lll3, mmm3_nnn3_oooo3} =
		new Cls1a(xx3, yy3, zz3);

	// Short: stays inline
	const {a, b} = fn1();
</script>
