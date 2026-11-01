<style>
	div {
		/* fits at 100: stays inline, the escaped space ending the last argument kept */
		background: f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, cccc\ );

		/* exceeds 100: the arguments wrap one-per-line, and the escaped space ending the last
		   argument stays on its line, ahead of the break */
		background: f(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
			ccccc\ 
		);

		/* fits at 100: stays inline, the escaped space ending the middle word kept */
		animation: aaaaaaaaaaaaaaaaaaaaaaaaaaaa bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\  ccccc;

		/* exceeds 100: the value fills, breaking at the separator after the escaped space,
		   which stays on its line */
		animation: aaaaaaaaaaaaaaaaaaaaaaaaaaaa bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\ 
			cccccc;
	}
</style>
