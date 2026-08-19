<script lang="ts">
	// A JSDoc cast owns its comment, so a freeze that replaces the value's doc with a
	// verbatim slice has to print that comment itself — the slice starts at the cast's `(`.
	const aaa =
		// prettier-ignore
		/** @type {T} */ (bbb  =  ccc);

	// The cast owns its comment from the line ABOVE its `(` too, and that line is the
	// author's: the freeze keeps it, exactly as the unfrozen cast does.
	const ddd =
		// prettier-ignore
		/** @type {T} */
		(eee  =  fff);

	// A redundant shell around the cast collapses into the cast's own preserved pair.
	const ggg =
		// prettier-ignore
		/** @type {T} */
		(hhh  =  iii);

	// The unfrozen twin, for contrast: same two placements, same two answers.
	const jjj = /** @type {T} */ (kkk = lll);

	const mmm =
		/** @type {T} */
		(nnn = ooo);
</script>
