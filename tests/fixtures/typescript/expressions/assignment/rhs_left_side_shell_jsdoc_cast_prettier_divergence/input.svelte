<script>
	// A JSDoc cast the author gave a line of its own, at the head of a member chain, hangs
	// the value under the operator at every assignment seam — however the cast was reached:
	// written there, or inside a grouping shell the chain prints mid-chain
	// (`unformatted_ours_mid_chain_shell`), where the cast's leaf still prints its comment
	// ahead of the chain's head.
	x =
		/** @type {T} */
		(f).b.c(zz);
	const y =
		/** @type {T} */
		(f)[0].c(zz);
	o.p =
		/** @type {T} */
		(f).b.d.c(zz);
	o = {
		k:
			/** @type {T} */
			(f).b.c(zz)
	};
</script>
