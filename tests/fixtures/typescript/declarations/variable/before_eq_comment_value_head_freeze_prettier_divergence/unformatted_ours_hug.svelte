<script lang="ts">
	// A comment before the `=` drops `= value` to a continuation line — and the
	// `=`→value gap keeps its OWN rule inside that continuation: an own-line directive
	// there still freezes the value, and still keeps its own line (trailing the `=` it
	// would be inert, so the next pass would drop the freeze).
	const aaa // c1
		=
		// prettier-ignore
		(bbb  =  ccc);

	// The same with no shell to re-synthesize.
	const ddd // c2
		=
		// prettier-ignore
		eee  +  fff;

	// A class field's `=` is the same gap one host over.
	class G {
		hhh // c3
			=
			// prettier-ignore
			(iii  =  jjj);
	}

	// So is an assignment expression's, with the author's operator on the continuation.
	kkk // c4
		=
		// prettier-ignore
		lll  +  mmm;

	// The rule is not the directive's: any own-line comment after the `=` leads the value
	// from its own line here, exactly as it does when nothing precedes the `=`.
	const nnn // c5
		=
		// c6
		ooo;

	// The control — a comment the author put ON the `=`'s line stays there.
	const ppp // c7
		= // c8
		qqq;

	// The value's clarity parens are the position's, and the gap's content cannot strip
	// them.
	const rrr // c9
		= (sss = ttt);

	// The run the value's own doc prints leads it here too. A JSDoc cast OWNS its comment,
	// so the gap's to-emit lookup cannot see it — but it is on the page, and the line the
	// author gave it survives exactly as it does outside the continuation.
	const uuu // c10
		=
		/** @type {Aaa} */
		(vvv = www);

	// The glued authoring of that cast is the control, and stays on the operator's line.
	const xxx // c11
		= /** @type {Aaa} */ (yyy = zzz);

	// A block glued to the operator hugs the value across the author's newline — the same
	// pull-up the ordinary arm makes, authored broken in `unformatted_ours_hug`.
	const aab // c12
		= /* c13 */
		bbc;

	// An enum member's `=` is the same gap one host over.
	enum Aac {
		Bbd // c14
			=
			// prettier-ignore
			ccd  +  dde
	}

	// So is a `for` header's init declarator, whose clause separator is a `;` rather than a
	// statement terminator.
	for (
		let eef // c15
			=
			// prettier-ignore
			ffg  +  ggh;
		eef < 10;
		eef++
	) {
		fn();
	}
</script>
