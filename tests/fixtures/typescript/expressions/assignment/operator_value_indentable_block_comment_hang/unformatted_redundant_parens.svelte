<script lang="ts">
	// An INDENTABLE multi-line block comment leading the value hangs the value under
	// the operator, whether or not it is glued to the value's own first token — the
	// rule is asked of every comment leading the value, so a grouping paren the
	// printer discards (the `unformatted_redundant_parens` variant) or a second
	// comment may stand between the two.

	// declarator
	const a = /**
	 * c
	 */ (x);

	// the indentable member at the head of a run, the value glued to its tail
	const b = /**
	 * c
	 */ /* c1 */ (x);

	// ...and at the tail of a run
	const c = /* c1 */ /**
	 * c
	 */ (x);

	// assignment expression
	let d;
	d = /**
	 * c
	 */ (x);

	// object property
	const e = {
		k: /**
		 * c
		 */ (x)
	};

	// class field
	class C {
		f = /**
		 * c
		 */ (x);
	}

	// Contrast: a PRESERVED (non-indentable) multi-line block keeps the value on the
	// operator's line, and so does a single-line block — at every one of the seams
	// above, and with a discarded paren standing between the comment and the value.
	const g = /* line1
	line2 */ (x);
	const h = /* c1 */ (x);
	const i = {
		k: /* line1
		line2 */ (x)
	};
</script>
