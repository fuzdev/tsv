<style>
	@media screen {
		 /* comment before nested atrule */
		@supports (display: grid) {
			 /* comment in nested atrule */
			div {
				display: grid;  /* comment in deeply nested rule */
			}
		}  /* comment after nested atrule */
	}
</style>
