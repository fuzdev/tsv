<!--
	A whitespace-collapsing container lays out block-style, but the break may only land where
	the compiler removes the whitespace ENTIRELY — between two non-text children. A boundary
	with a TEXT child lands INSIDE that text node, where the whitespace collapses to a rendered
	space instead of vanishing, so the authored boundary is reproduced exactly: glued stays
	glued, and an authored space is carried by the line break rather than written twice. The
	container's own edges are always trimmed (the compiler strips them). Cases below, in order:
	a lone text child; text glued after an element; text glued before an element; an authored
	space before the text; the same glued case in a <datalist>; two glued elements (control —
	their boundary is whitespace-only, so it is removed entirely and the break is free).
-->
<select> text </select>
<select>
	<option>a</option>text
</select>
<select>
	text<option>a</option>
</select>
<select>
	<option>a</option>
	text
</select>
<datalist>
	<option>b</option>text
</datalist>
<select>
	<option>a</option>
	<option>b</option>
</select>
