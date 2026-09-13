<style>
	div {
		/* a comment inside an opaque value is kept where it was written */
		filter: progid:X.Y(a=1.50) /* c */ progid:Z.W(b=2);
		/* a trailing comment is kept, and nothing trails the value */
		filter: progid:X.Y(a=1.50) /* c */;
		/* a comment ahead of `!important` is kept too, like the one after it */
		filter: progid:X.Y(a=1.50) /* c */ !important /* d */;
		/* a comment glued to its neighbours on both sides is content on both formatters */
		filter: progid:X.Y(a=1.50)/* c */progid:Z.W(b=2);
		/* the trigger is the value's first bytes: a `progid:` after another member is an ordinary
		   glued token kept whole, and the numbers around it normalize */
		filter: alpha(opacity=50) progid:X.Y(a=1.50) 1.5px;
		/* the same value in a CUSTOM property, whose top-level `:` postcss parses instead of
		   rejecting: the lowercase prefix still reaches the opaque arm, so the value freezes on
		   both sides and the colon keeps its glue */
		--a: progid:X.Y(a=1.50);
		/* any other case of the prefix is the same opaque value to tsv, and an ordinary
		   top-level colon to prettier, which spaces it */
		--b: PROGID:X.Y(a=1.50);
	}
</style>
