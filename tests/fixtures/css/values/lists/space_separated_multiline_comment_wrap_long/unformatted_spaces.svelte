<style>
	div {
		/* the word after a multi-line comment's last line does not fit on that line and drops to a */
		/* fresh line, where the fill continues: 100 columns with the `;` keeps the tail beside it */
		background:   1px   /* a
		bbbbbbbb */
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa   2px;
		/* 102 columns: the tail wraps beneath the dropped word */
		background:   1px   /* a
		bbbbbbbb */
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
	}
</style>
