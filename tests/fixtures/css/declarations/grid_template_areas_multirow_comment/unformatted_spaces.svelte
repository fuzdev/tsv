<style>
	div {
		/* trailing comment after the last row */
		grid-template-areas:
			'a a'
			'b c'   /* c */  ;
		/* comment ending a row */
		grid-template-areas:
			'a a'   /* c */
			'b c';
		/* comment on a line of its own between rows */
		grid-template-areas:
			'a a'
			   /* c */
			'b c';
		/* comment opening a row */
		grid-template-areas:
			'a a'
			/* c */   'b c';
		/* a run of two comments ends a row together */
		grid-template-areas:
			'a a'  /* c1 */   /* c2 */
			'b c';
		/* a multi-line comment ends the row it opens on */
		grid-template-areas:
			'a a'   /* c
			d */
			'b c';
		/* leading run stays on the colon's line; the rows break beneath it */
		grid-template-areas: /* c */
			'a a'
			'b c';
		/* leading run with inline rows stays inline */
		grid-template-areas: /* c */   'a a'   'b c';
		/* comment between inline rows stays inline */
		grid-template-areas:  'a a'   /* c */   'b c';
		/* a comment on a line of its own after inline rows is a row of its own */
		grid-template-areas:
			'a a'   'b c'
			/* c */  ;
		/* !important follows the last row */
		grid-template-areas:
			'a a'
			'b c'   /* c */   !important;
		/* a row is a line, not a fill: a long row never wraps */
		grid-template-areas:
			'aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa'   /* cccccccccc */
			'b c';
		/* the `grid` shorthand takes the same rule */
		grid:
			'a a'   /* c */
			'b c';
	}
</style>
