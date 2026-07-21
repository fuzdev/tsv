<!--
	A boundary break on ONE side is not an expansion signal. Content-boundary whitespace is
	render-free under Svelte 5, and a component expands only when the author broke BOTH content
	boundaries (see ../expr_child_multiline/). Authored with just the opening newline, the content
	collapses back inline. Prettier keeps the half-authored break and dangles the closing tag to
	hold the content against it — see prettier_variant_opening_newline.
-->
<Comp>
	  { expr }</Comp
>

<!-- Same with several expression children -->
<Comp>
	  { expr1 }  { expr2 }  { expr3 }</Comp
>
