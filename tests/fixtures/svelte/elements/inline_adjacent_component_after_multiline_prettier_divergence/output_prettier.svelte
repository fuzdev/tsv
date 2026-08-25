<!--
	The adjacent separator — the whitespace-only node between two non-text siblings — before a
	COMPONENT whose predecessor renders MULTILINE, spelled as a SPACE. The component hugs the
	predecessor's closing tag exactly as an inline element in that position does (the boundary is
	measured from the closing tag's column, where it fits); prettier splits it off. The NEWLINE
	spelling is held by both formatters — the layout-keyed rule sibling_newline_after_multiline
	pins — so the boundary is dual-stable here (variant_newline).

	The cases vary only what made the container multiline — the run and its separators are the
	same in each — so the component's boundary does not depend on the container's cause. The
	prose-free control keeps its authored line: with no fill to reflow into, the newline is the
	author's only structure.
-->
<p>
	text1
	<span>
		<b>inline1</b>
	</span>
	<Comp />
</p>

<!-- control: multiline STRUCTURALLY (a block child), not by the run's own newlines -->
<div>
	text1
	<span>
		<b>inline1</b>
	</span>
	<Comp />
	<div>block1</div>
</div>

<!-- control: a prose-free run, which has no fill to reflow into, keeps its authored line -->
<p>
	<span>
		<b>inline1</b>
	</span>
	<Comp />
</p>
