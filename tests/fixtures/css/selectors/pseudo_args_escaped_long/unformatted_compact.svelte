<style>
	/* fits at 100 — stays inline; the trailing escape's terminator drops before `)` */
	div:is(.aaaaaa,.bbbbbb,.cccccc,.dddddd,.eeeeee,.ffffff,.ggggggggggggggggggggggggggg,.\41){color:red;}
	/* exceeds 100 — wraps one-per-line; the last arg's escape absorbs the line
	   break as its terminator, which must drop before the closing `)` */
	div:is(.aaaaaa,.bbbbbb,.cccccc,.dddddd,.eeeeee,.ffffff,.gggggggggggggggggggggggggggg,.\41){color:blue;}
</style>
