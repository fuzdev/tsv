<style>
	div {
		/* 100 chars: the value fits the colon's line and stays inline */
		grid-template-columns: [a] aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1fr [b] 2fr;
		/* 101 chars: the value drops beneath the colon as a row */
		grid-template-columns: [a] aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1fr [b] 2fr;
		/* a long track list packs greedily into rows beneath the colon, each row at most 100 chars */
		grid-template-columns: [abcd] 1fr [b] 2fr [c] 3fr [d] 4fr [e] 5fr [f] 6fr [g] 7fr [h] 8fr [i] 9fr [j] 10fr [k] 11fr [l] 12fr [m] 13fr;
		/* the leading comment run stays on the colon's line, outside the rows */
		grid-template-columns: /* c */ [a] aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1fr [b] 2fr;
		/* a comment inside the value rides the row */
		grid-template-columns: [a] aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa /* c */ 1fr;
		/* area strings wrap the same way */
		grid-template-areas: 'aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaaaaaa aaaaaa' 'b';
		/* the grid shorthand with a slash */
		grid: 'abcdef' 1fr 'b' 2fr 'c' 3fr 'd' 4fr 'e' 5fr 'f' 6fr 'g' 7fr 'h' 8fr 'i' 9fr 'j' 10fr 'k' 11fr / auto;
	}
</style>
