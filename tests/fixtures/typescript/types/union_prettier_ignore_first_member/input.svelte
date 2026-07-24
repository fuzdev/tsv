<script lang="ts">
	// own-line line comment before the first member freezes only that member;
	// the reformatted union collapses inline when the members are short
	type A =
		// prettier-ignore
		(a1&a2) | (b1 & b2);

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	type B =
		/* prettier-ignore */
		(a1&a2) | (b1 & b2);

	// a fully redundant paren around the frozen member is still dropped (paren-transparent)
	type C =
		// prettier-ignore
		a1 | b;

	// long union stays broken at natural width; the frozen first member keeps its verbatim
	type D =
		// prettier-ignore
		| (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
		| cccccccccccccccccccccccccccccc
		| dddddddddddddddddddddddddddddd;
</script>
