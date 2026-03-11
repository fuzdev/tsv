<script lang="ts">
	fn1(async () => {
		fn1(async () => {
			// call head visual = 100 at indent 3: default layout (call expands args)
			const {a1_b1_c1, d1_e1_f1, g1_h1_i1, j1_k1_l1, m1_n1_o1, ppp_qqq_rrr_sss_ttt_uuu_v} = fn1.fn2(
				await fn3('./aaa'),
			);

			// call head visual = 101 at indent 3: fluid layout (break after =)
			const {a2_b2_c2, d2_e2_f2, g2_h2_i2, j2_k2_l2, m2_n2_o2, ppp_qqq_rrr_sss_ttt_uuu_vv} =
				fn1.fn2(await fn3('./aaa'));

			// short - everything fits on one line
			const {a3, b3, c3} = fn1.fn2(await fn3());
		});
	});
</script>
