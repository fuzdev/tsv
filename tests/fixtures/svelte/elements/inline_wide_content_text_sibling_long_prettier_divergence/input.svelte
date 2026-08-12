<!--
	A wide inline element whose PROSE content overflows, followed by text and then ANOTHER
	inline element. tsv wraps the content within print width (the printWidth-hard-limit
	divergence) and the non-terminal text between (`mid`) hugs the intact closing tag: the
	tail boundary is a per-width fill decision however the element came to be multiline
	(contrast inline_wide_element_content_tail_long, the element-content twin at the exact
	100/101 boundary). The second case swaps the preceding text for an inline ELEMENT
	sibling, so the wide element sits in its inline-sibling wrap — the wrap's other side
	flows, so the tail takes the same per-width hug; only a NON-flowing other side (a
	comment or a control-flow block) keeps the outside-in joint measurement, see
	inline_sibling_drop_tail_flow_long. prettier groups the boundary WITH the element, so
	it re-breaks the hug and keeps the own-line form stable.
-->
<p>
	<a href="#">
		alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike november oscar
	</a> mid <b>x</b>
</p>

<p>
	<span>a</span>
	<b>
		alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike november oscar
	</b> mid <i>z</i>
</p>
