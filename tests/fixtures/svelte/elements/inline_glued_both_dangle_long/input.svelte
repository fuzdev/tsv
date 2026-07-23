<!--
	An inline element glued to text on BOTH sides (no whitespace either side) on an
	over-width line cannot break before or after, so tsv dangles the element's closing
	`>` — `<span>x</span⏎>` — the render-safe, tag-internal break (whitespace inside an
	end tag is ignored, so the AST is byte-identical). This is the sibling-`>` dangle
	(already used when an inline element is glued to a following element/block) firing onto
	following glued text, and the no-trailing-space analog of the trailing-space wrap. It
	applies while the dangle line (`prefix<span>content</span`) still fits ≤100; the
	content stays inline. Both formatters agree. Cases: fits inline@100 (control); 101
	(breaks + dangles); larger content that still fits the dangle line (dangles); the widest
	prefix whose dangle line is exactly 100 (dangles — one wider block-styles, see
	inline_glued_before_long_prettier_divergence).
-->
<p>
	ffffffffffffffffffffffffffffffffffffffffff<span>x</span>bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
</p>
<p>
	ffffffffffffffffffffffffffffffffffffffffff<span>x</span
	>bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
</p>
<p>
	ffffffffffffffffffff<span>cccccccccccccccccccccccccccccccccccccccc</span
	>bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
</p>
<p>
	fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<span>x</span
	>bbbbbbbbbb
</p>
