<script lang="ts">
	// an own-line directive in the `(`→init gap freezes the whole init clause
	for (
		// prettier-ignore
		i  =  0;
			i  <  10;
		i  ++
	) {
			fn();
	}

	// the `;`→test gap freezes the test clause; the other clauses normalize
	for (
		let   i   =   0;
		// prettier-ignore
		i  <  10;
			i  ++
	) {
		fn();
	}

	// the `;`→update gap freezes the update clause
	for (
		let  i  =  0;
		i   <   10;
		// prettier-ignore
		i  ++
	) {
			fn();
	}

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	for (
		/* prettier-ignore */
		i  =  0;
		i  <  10;
			i  ++
	) {
		fn();
	}

	// a clause that is a sequence freezes WHOLE: the directive leads the sequence node,
	// so every operand rides inside the verbatim slice
	for (
		// prettier-ignore
		i  =  0,
			j  =  1;
		i   <   10;
		i  ++
	) {
		fn();
	}

	// a multi-line frozen clause keeps its verbatim layout; the header `;` is parent-owned
	for (
		i   =   0;
		// prettier-ignore
		fn(
			a,
				b
		);
		i  ++
	) {
			fn();
	}

	// an assignment test's clarity parens are the printer's, so they stay OUTSIDE the
	// frozen slice — the same shell the unfrozen clause prints
	for (
		i   =   0;
		// prettier-ignore
		(  jjj  =  kkk  );
		i  ++
	) {
			fn();
	}
</script>
