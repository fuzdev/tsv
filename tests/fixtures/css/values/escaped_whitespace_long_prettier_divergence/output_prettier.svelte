<style>
	div {
		/* 100 chars: fits exactly, stays inline (the escaped space is content, not a wrap point) */
		grid-template-areas: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa bbbb cccc dddd xxxxx\yyyyy;

		/* 101 chars: must break — but NOT on the escaped space, which is inside an ident */
		grid-template-areas:
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa bbbb cccc dddd
			xxxxx\yyyyy;
	}
</style>
