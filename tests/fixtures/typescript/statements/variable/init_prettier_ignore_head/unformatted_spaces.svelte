<script lang="ts">
	// an own-line directive in the `=`→initializer gap freezes the whole initializer
	const aaa   =
		// prettier-ignore
		bbb  +  ccc;

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	const ddd   =
		/* prettier-ignore */
		eee  +  fff;

	// a multi-line frozen initializer keeps its verbatim layout
	const ggg   =
		// prettier-ignore
		{
				hhh:   1,
			iii: 2
		};

	// an assignment initializer's clarity parens are the printer's, so they stay OUTSIDE the
	// frozen slice
	const jjj   =
		// prettier-ignore
		(kkk  =  lll);

	// a destructuring binding takes its own layout and freezes the same way
	const {   mmm   }   =
		// prettier-ignore
		nnn  ||  ooo;

	// a type-annotated binding is unaffected by the freeze
	const ppp:   number   =
		// prettier-ignore
		qqq  +  rrr;

	// a sibling declaration the freeze does not reach still normalizes
	const sss   =
		// prettier-ignore
		ttt  +  uuu;
	const vvv   =   www   +   xxx;
</script>
