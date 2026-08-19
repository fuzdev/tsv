<script lang="ts">
	async function fn1() {
		// An own-line directive in the `new`→callee gap freezes the CALLEE. The type
		// arguments and the argument list sit past the callee's span, so they stay
		// parent-owned and still normalize.
		const aaa = new
			// prettier-ignore
			Bbb.Ccc(ddd);

		const eee = new
			// prettier-ignore
			Fff<Ggg>(hhh);

		// The `await`→operand gap is the same gap one keyword over and takes the same
		// freeze — here over the whole call, arguments included.
		const iii = await
			// prettier-ignore
			fn2(  jjj  );

		// An own-line block comment behaves identically — placement keys the freeze, not
		// the spelling.
		const kkk = new
			/* prettier-ignore */
			Lll.Mmm(nnn);

		const ooo = await
			/* prettier-ignore */
			fn2(  ppp  );

		// An operand whose parens are REQUIRED keeps them around the frozen slice: they
		// are the printer's, not the author's, so they ride outside it.
		const qqq = new
			// prettier-ignore
			(rrr  ??  sss)(ttt);

		const uuu = await
			// prettier-ignore
			(vvv  ??  www);

		// A directive the author put on the keyword's line is INERT under the placement
		// floor: the comment keeps the line it was written on and the operand normalizes.
		const xxx = new // prettier-ignore
			Yyy.Zzz(a1);

		const b1 = await // prettier-ignore
			fn2(c1);
	}
</script>
