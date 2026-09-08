<style>
	div {
		/* a multi-line comment's FIRST line is measured from where the comment starts: at 100 columns
		   it stays on the colon's line */
		background: 1px
			/* aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		b */ 2px;
		/* 101 columns: the comment drops to a fresh line; the word after its last line stays beside
		   it, measured from where the comment ends */
		background: 1px
			/* aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		b */ 2px;
		/* a first line wider than the fresh line too: the comment still drops, and the word after
		   its last line still stays beside it */
		background: 1px
			/* aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		b */
			2px;
		/* a LEADING comment whose first line overruns the colon's line: it drops to a fresh line as
		   the wide first item every fill drops, the word after it beside its last line */
		margin: /* aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		b */ 1px;
	}
</style>
