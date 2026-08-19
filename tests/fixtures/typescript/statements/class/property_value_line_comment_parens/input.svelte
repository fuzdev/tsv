<script lang="ts">
	class A {
		// A class field's value keeps the clarity parens its position supplies, whatever
		// the `=`→value gap holds: an assignment as a value prints parenthesized.
		b = // c1
			(ccc = ddd);

		// The own-line placement of the same comment answers alike.
		e =
			// c2
			(fff = ggg);

		// Every field flavour is the same position.
		static h = // c3
			(iii = jjj);

		accessor k = // c4
			(lll = mmm);

		#n = // c5
			(ooo = ppp);

		// A frozen value takes the same pair — the parens are the printer's, so they sit
		// outside the verbatim slice.
		q =
			// prettier-ignore
			(rrr  =  sss);

		// The control: a block comment in that gap keeps the value on the `=` line, and the
		// pair with it.
		t = /* c6 */ (uuu = vvv);
	}
</script>
