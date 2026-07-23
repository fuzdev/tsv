<script lang="ts">
	// first union member is a parenthesized union with a leading line comment inside the
	// parens, kept inside the member - the comment takes its own line inside the parens,
	// above the inline union
	type First =
		| // c
		  (A | B)
		| C;

	// first member is a retained-paren intersection with a trailing object, which
	// supplies its own aligned layout - the comment is kept inside the parens the same
	// way, hugging the `(` above the intersection
	type FirstIntersection =
		| // c
		  (A & {
				a: 1;
		  })
		| C;

	// a leading line comment inside a LATER member's parens is kept inside the parens,
	// the same as the first member - tsv associates it with the member it documents
	// rather than hoisting it out (prettier hoists it to its own line above the member)
	type Mid =
		| A
		// c
		| (B | C)
		| D;

	// the keep-inside rule holds for every RETAINED-paren member kind, not just unions:
	// a later paren-function member keeps the comment inside (prettier trails it on the
	// previous member and keeps the member inline)
	type MidFunction =
		| A // c
		| (() => B)
		| D;

	// a later paren-conditional member - keeping the comment inside forces the paren
	// group open, so the conditional breaks its branches; prettier hoists the comment
	// out and keeps the conditional inline
	type MidConditional =
		| A // c
		| (B extends C ? D : E)
		| F;

	// a later paren-intersection member (no trailing object) - same keep-inside, the
	// forced-open paren breaks the intersection
	type MidIntersection =
		| A // c
		| (B & C)
		| D;
</script>
