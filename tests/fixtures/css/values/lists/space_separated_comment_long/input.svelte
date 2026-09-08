<style>
	div {
		/* trailing comment: 100 columns with the `;` stays inline */
		background: 1px aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa /* c */;
		/* 102 columns: the trailing comment is a fill item and wraps to a continuation line */
		background: 1px aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			/* c */;
		/* leading comment: 100 columns stays inline */
		background: /* c */ 1px aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2px;
		/* 102 columns: the last word wraps */
		background: /* c */ 1px aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
		/* mid comment: 100 columns stays inline */
		background: 1px /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2px;
		/* 102 columns: the comment stays with the word before it and the last word wraps */
		background: 1px /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
		/* a mid comment is a fill item of its own: when the word before it fills the line, the */
		/* comment wraps with the tail */
		background: 1px aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			/* c */ 2px;
		/* a multi-comment run is two fill items; 102 columns wraps the tail */
		background: 1px /* c1 */ /* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
		/* custom property: 100 columns stays inline */
		--prop: 1px /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2px;
		/* 102 columns wraps with the same continuation indent */
		--prop: 1px /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
		/* a long property moves the boundary with it: 100 columns stays inline */
		prop000000000000000000000000000: 1px /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2px;
		/* 102 columns wraps */
		prop000000000000000000000000000: 1px /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
		/* a leading comment run is a fill item too: at 100 columns it fills the first line exactly */
		/* with the word after it and the tail wraps; one column more and the run and the word part */
		/* (space_separated_leading_comment_long_prettier_divergence) */
		prop000000000000000000000000000: /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
	}
</style>
