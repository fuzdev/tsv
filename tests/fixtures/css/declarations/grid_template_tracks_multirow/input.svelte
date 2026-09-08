<style>
	div {
		/* single track list - stays inline */
		grid-template-columns: [a] 1fr [b] 2fr;
		/* named lines - each row on its own line */
		grid-template-columns:
			[a] 1fr
			[b] 2fr;
		/* a lone line name is a row of its own */
		grid-template-columns:
			[a]
			1fr
			[b] 2fr;
		/* rows of two tracks each */
		grid-template-rows:
			[a] auto
			[b c] 1fr
			[d];
		/* function members - each on its own line */
		grid-template-columns:
			repeat(2, 1fr)
			auto;
		grid-template-columns:
			minmax(100px, 1fr)
			minmax(200px, 2fr)
			auto;
		/* a row of several tracks stays a row; the next row breaks */
		grid-template-columns:
			[a] 1fr [b] 2fr
			[c] 3fr;
		/* the grid shorthand: areas with tracks, and a slash on the last row */
		grid:
			'a' 1fr
			'b' 2fr / auto;
		/* the slash opens its own row */
		grid:
			'a' 1fr
			'b' 2fr
			/ auto 1fr;
		/* grid-template mixes area strings and tracks */
		grid-template:
			'a a' 1fr
			'b c' 2fr / 1fr
			1fr;
		/* a comment ends a row */
		grid-template-columns:
			repeat(2, 1fr) /* c */
			auto;
		/* leading run stays on the colon's line; the rows break beneath it */
		grid-template-columns: /* c */
			[a] 1fr
			[b] 2fr;
		/* a multi-line comment ends the row it opens on */
		grid-template-columns:
			[a] 1fr /* c
			d */
			[b] 2fr;
		/* numbers and units normalize on a row like anywhere else */
		grid-template-columns:
			[a] 1.5fr
			[b] 2fr;
		/* track sizes of every kind are rows alike */
		grid-template-columns:
			fit-content(100px)
			min-content
			max-content;
		grid-template-columns:
			50%
			50%;
		/* empty line names */
		grid-template-columns:
			[] 1fr
			[] 2fr;
		/* subgrid with a line-name list */
		grid-template-columns:
			subgrid [a]
			repeat(auto-fill, [b]);
		/* the grid shorthand's auto-flow form */
		grid:
			auto-flow dense
			/ 1fr 2fr;
		/* grid-template with line names around the area strings */
		grid-template:
			[a] 'x' 1fr [b]
			[c] 'y' 2fr [d]
			/ auto;
		/* !important is not a track: it never opens a row */
		grid-template-columns: 1fr 2fr !important;
		/* only grid and grid-template* read the author's lines */
		grid-auto-rows: 1fr 2fr;
		/* a long row never wraps as a fill; a function on it breaks by its own group */
		grid-template-columns:
			repeat(auto-fill, minmax(100px, 1fr)) repeat(auto-fill, minmax(100px, 1fr)) repeat(
				auto-fill,
				minmax(100px, 1fr)
			)
			auto;
	}
</style>
