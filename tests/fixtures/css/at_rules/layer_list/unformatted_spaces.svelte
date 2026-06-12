<style>
	/* comma spacing preserved verbatim (matches prettier + Svelte AST), not normalized */
	@layer   base,  components  ;
	@layer   theme , layout  ;
	@layer   a,b  ;
	/* single-space form is already canonical — non-diverging sibling / regression guard */
	@layer   reset, base, utilities  ;
	/* block form is unaffected */
	@layer   base   {
		h1 {
			font-size: 2rem;
		}
	}
</style>
