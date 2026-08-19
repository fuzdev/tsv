<script lang="ts">
	// an own-line directive in a `case`→test gap freezes the whole test
	switch (aaa) {
		case
			// prettier-ignore
			bbb  +  ccc:
			fn(ddd);
	}

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	switch (eee) {
		case
			/* prettier-ignore */
			fff  +  ggg:
			fn(hhh);
	}

	// the clarity parens a `case` test takes are the printer's, so the frozen inner keeps them
	switch (iii) {
		case
			// prettier-ignore
			(jjj  =  kkk):
			fn(lll);
	}

	// the `:` stays parent-owned and a sibling case the freeze does not reach still normalizes
	switch (mmm) {
		case
			// prettier-ignore
			nnn  +  ooo:
			fn(ppp);
		case qqq + rrr:
			fn(sss);
	}

	// a directive the author put on the `case` line is INERT under the placement floor: the
	// comment keeps the line it was written on and the test normalizes
	switch (a2) {
		case // prettier-ignore
			b2 + c2:
			fn(d2);
	}

	// the block spelling of the same placement, likewise inert
	switch (e2) {
		case /* prettier-ignore */ f2 + g2:
			fn(h2);
	}

	// the head→`:` gap is a different gap and keeps its own answer
	switch (ttt) {
		case
			// prettier-ignore
			uuu  +  vvv /* c */:
			fn(www);
	}
</script>
