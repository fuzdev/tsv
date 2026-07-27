<script lang="ts">
	// an own-line directive in the `=`→RHS gap freezes the whole RHS
	aaa =
		// prettier-ignore
		bbb  +  ccc;

	// a two-segment chain freezes the innermost RHS
	ddd = eee =
		// prettier-ignore
		fff  +  ggg;

	// a three-segment chain takes the chain layout and freezes the same way
	hhh =
		iii =
		jjj =
			// prettier-ignore
			kkk  +  lll;

	// a destructuring target never breaks after the operator, and freezes the same way
	({ mmm } =
		// prettier-ignore
		nnn  ||  ooo);

	// a compound operator is the same head
	ppp +=
		// prettier-ignore
		qqq  +  rrr;

	// an object property's `:`→value gap is the same head
	const obj = {
		sss:
			// prettier-ignore
			ttt  +  uuu,
		vvv: www + xxx
	};

	// a class field's `=`→value gap is the same head
	class Single {
		yyy =
			// prettier-ignore
			zzz  +  aab;

		static abb =
			// prettier-ignore
			acc  +  add;
	}
</script>
