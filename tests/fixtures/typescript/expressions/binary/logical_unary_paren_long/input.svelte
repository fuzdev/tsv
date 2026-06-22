<script lang="ts">
	// short logical in !!() - fits inline
	const a = !!(aa.bb && cc.dd);

	// 100 chars - stays inline (same for ||, ??)
	const b = $derived(
		!!(aa_bb.cc_dd_ee_ff_gg_hh && aa_bb.ii_jj_kk.get(aa_bb.cc_dd_ee_ff_gg_hh)?.ll_mm_nn_oo_pp_qq_rr)
	);

	// 101 chars - outer breaks, inner stays on one line
	const c = $derived(
		!!(
			aa_bb.cc_dd_ee_ff_gg_hh && aa_bb.ii_jj_kk.get(aa_bb.cc_dd_ee_ff_gg_hh)?.ll_mm_nn_oo_pp_qq_rrs
		)
	);

	// 101 chars inner - outer and inner both break
	const d = $derived(
		!!(
			aa_bb.cc_dd_ee_ff_gg_hh &&
			aa_bb.ii_jj_kk.get(aa_bb.cc_dd_ee_ff_gg_hh)?.ll_mm_nn_oo_pp_qq_rr_ss
		)
	);

	// 100 chars inner - outer breaks, inner stays on one line
	const e = $derived(
		!!(
			aa_bb.cc_dd_ee_ff_gg_hh || aa_bb.ii_jj_kk.get(aa_bb.cc_dd_ee_ff_gg_hh)?.ll_mm_nn_oo_pp_qq_rr_s
		)
	);

	// 3-operand - all break together when outer breaks
	const f = $derived(
		!!(
			aa_bb.cc_dd_ee_ff_gg_hh ||
			aa_bb.ii_jj_kk.get(aa_bb.cc_dd_ee_ff_gg_hh)?.ll_mm_nn ||
			oo_pp_qq_rr_ss
		)
	);
</script>
