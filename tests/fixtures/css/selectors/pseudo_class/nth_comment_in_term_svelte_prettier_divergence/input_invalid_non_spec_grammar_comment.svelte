<style>
	/* the bare An+B terms :is()/:not() over-accept take Svelte's REGEX_NTH_OF grammar,
	   which stays comment-blind for parseCss parity - so a split term is not a term
	   there, and :not()'s strict list rejects it in both parsers */
	a:not(2n /* c */ + 1) {
		color: red;
	}
</style>
