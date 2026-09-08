<style>
	div {
		/* the first item fills the colon's line to exactly 100 columns; the tail wraps beneath it */
		background: 'aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaa'
			2px;
		/* one column more: the first item cannot fit on the colon's line and drops to a fresh line */
		background:
			'aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaa'
			2px;
		/* the same boundary with a comment in the value: 100 columns */
		background: 'aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaa'
			/* c */ 2px;
		/* 101 columns: the first item drops, the comment rides the tail */
		background:
			'aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaa'
			/* c */ 2px;
	}
</style>
