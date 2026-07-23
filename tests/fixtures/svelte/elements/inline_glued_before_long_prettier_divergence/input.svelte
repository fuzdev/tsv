<!--
	Two glued-before shapes where tsv does NOT dangle the closing `>` (unlike the
	dangle case, see inline_glued_both_dangle). The dangle only applies while the
	dangle line (`prefix<span>content</span`) fits ≤100; past that, and when there is
	a trailing space to wrap at instead, tsv resolves the over-width line differently
	from prettier. Cases below.
-->

<!-- glued both sides, dangle line at 101 (one past the widest that dangles) → block-style,
     since dangling the `>` would leave `…<span>x</span` over-width; prettier double-dangles
     (`<span⏎>x</span⏎>`). This is the block-style side of the dangle boundary — the
     dangle@100 case is in inline_glued_both_dangle_long. -->
<p>
	ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<span>
		x
	</span>bbbbbbbbbb
</p>

<!-- glued before, space after, fits at exactly 100 → stays inline (control) -->
<p>
	fffffffffffffffffffffffffffffffffffffffff<span>x</span> bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
</p>

<!-- glued before, space after, 101 → wraps at the trailing space; prettier keeps the 101 line -->
<p>
	fffffffffffffffffffffffffffffffffffffffff<span>x</span>
	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
</p>
