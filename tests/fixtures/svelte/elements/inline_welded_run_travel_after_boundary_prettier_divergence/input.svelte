<!--
	A welded run (`var(` glued to a MULTILINE component) whose LEADING boundary is one the fill
	owns as its own `line` — after a tag, or after an element. The run travels to a fresh line
	rather than opening the component's tag mid-line, the same answer the text-predecessor
	control below already reaches: what precedes the run cannot change where it breaks.

	Two properties of the shape are load-bearing. The component is multiline because the AUTHOR
	wrote it so and its content is glued (`--value_{expr1}` has no whitespace seam to reflow at),
	which is the Tier-2 signal both formatters honor — the flat control pins that the collapse is
	available whenever it is authored. And the run FITS on one line, which is what isolates the
	rule: a run too wide to fit is broken by the ordinary width measurement, and would travel
	with or without the forced-break rule this fixture is about.
-->

<!-- tag predecessor -->
<p>
	{expr1} =
	var(<Comp name="value_{expr1}">
		--value_{expr1}
	</Comp>)
</p>

<!-- control: the same document authored FLAT — the collapse is the author's to make, and both
	formatters keep it, so the multiline cases above are authored structure rather than a refusal
	to collapse -->
<p>{expr1} = var(<Comp name="value_{expr1}">--value_{expr1}</Comp>)</p>

<!-- breaking-element predecessor: the element's own wrap does not answer this boundary -->
<p>
	<input
		class="class1 class2"
		data-attr1="value1"
		data-attr2="value2"
		data-attr3="value3"
		bind:value={expr1}
	/> =
	var(<Comp name="value_{expr1}">
		--value_{expr1}
	</Comp>)
</p>

<!-- control: a text predecessor, which already travels -->
<p>
	text1 =
	var(<Comp name="value_{expr1}">
		--value_{expr1}
	</Comp>)
</p>

<!-- control: a SPACED run after a tag, which already travels -->
<p>
	{expr1} text1
	<Comp name="value_{expr1}">
		--value_{expr1}
	</Comp>
</p>
