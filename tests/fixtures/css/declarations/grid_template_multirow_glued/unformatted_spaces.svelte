<style>
	div {
		/* a `/` written glued between the last row and the column list stays glued */
		grid-template  :
			'a a a'   40px
			'b c c'   40px/1fr   1fr  1fr  ;
		/* and one written spaced stays spaced */
		grid-template  :
			'a a a'   40px
			'b c c'   40px  /   1fr   1fr  1fr  ;
		grid  :
			[row1-start]   'a a'   1fr
			[row2-start]   'b b'   2fr  /   auto   1fr  ;
	}
</style>
