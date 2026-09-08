<style>
	div {
		/* a comment's interior newline restarts the column: the word after its last line fits when
		   that line is 100 columns, measured from where the comment actually ends */
		background: 1px /* a
		bbbbbbbb */
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2px;
		/* the fill continues past the comment at the real column: a short word stays on the
		   comment's line, the long one after it wraps and takes the tail with it */
		background: 1px /* a
		bbbbbbbb */ 2px
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3px;
	}
</style>
