<style>
	/* a comment in the prelude is preserved at every gap, padded by one space on each side:
	leading, after the name, before and after a comma, in a combinator gap, and trailing */
	@custom-selector   /* c1 */   :--a   h1  ,  h2  ;
	@custom-selector   :--b   /* c2 */   h1  ,  h2  ;
	@custom-selector   :--c   h1   /* c3 */   ,   h2  ;
	@custom-selector   :--d   h1   ,   /* c4 */   h2  ;
	@custom-selector   :--e   h1   /* c5 */   >   h2  ,  h3  ;
	@custom-selector   :--f   h1  ,  h2   /* c6 */   ;
</style>
