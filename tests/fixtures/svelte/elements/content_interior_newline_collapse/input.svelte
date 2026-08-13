<!--
	An authored newline INSIDE a content text is the fill's own wrap point, not an expansion
	signal: the run reflows per width, for every container kind alike. Only the BOUNDARY runs
	carry air (inline_boundary_air / boundary_air_one_sided), and only a run with no prose has
	no fill to reflow into (the control).
-->

<!-- block: the interior newline reflows, the element collapses -->
<div>
	<p>text1 text2 <b>x</b></p>
</div>

<!-- component: the same -->
<div>
	<Comp>text1 text2 <b>x</b></Comp>
</div>

<!-- inline element: the same -->
<div>
	<span>text1 text2 <b>x</b></span>
</div>

<!-- control: a prose-free run has no fill — its authored lines are structure and stay -->
<div>
	<p>
		<b>inline1</b>
		<b>inline2</b>
	</p>
</div>
