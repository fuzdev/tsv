<style>
	div {
		/* a comment inside a function member stays inside it; the row still breaks */
		grid-template-columns:
			repeat(2, /* c */ 1fr)
			auto;
		grid-template-columns:
			minmax(100px /* c */, 1fr)
			auto;
		grid-template-columns:
			1fr
			var(--a, /* c */ 2fr);
		grid-template-columns:
			1fr
			calc(1px + /* c */ 2px);
		/* a comment inside line names */
		grid-template-columns:
			[a /* c */ b] 1fr
			[c] 2fr;
		/* a comment ends the function's row; another trails the last row */
		grid-template-columns:
			repeat(2, 1fr) /* c */
			auto /* c */;
		/* a comment ahead of !important on the last row */
		grid-template-columns:
			1fr
			2fr /* c */ !important;
		/* a comment on the slash row of the grid shorthand */
		grid:
			'a' 1fr
			/* c */ / auto;
		/* the leading run and a comment inside a member together */
		grid-template-columns: /* c1 */
			repeat(2, /* c2 */ 1fr)
			auto;
		/* a comment ending a wide row */
		grid-template-columns:
			repeat(auto-fill, minmax(100px, 1fr)) repeat(auto-fill, minmax(100px, 1fr)) /* cccccccccc */
			auto;
	}
</style>
