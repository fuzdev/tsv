<!--
	A frozen element whose end tag is implied — by its parent's close or by the next sibling —
	ends its span at that boundary, so the whitespace in front of it is inside the frozen slice.
	The slice gives up that trailing whitespace run, and the printer prints the boundary once,
	at its own indentation.
-->

<ul>
	<!-- prettier-ignore -->
	<li>a
</ul>

<div>
	<ul>
		<!-- prettier-ignore -->
		<li>a
	</ul>
</div>

<ul>
	<!-- prettier-ignore -->
	<li>a
	<li>b</li>
</ul>

<table>
	<tbody>
		<!-- prettier-ignore -->
		<tr><td>a
	</tbody>
</table>

<p>
	<!-- prettier-ignore --><b>a
</p>

<!--
	Through a `<pre>` the run is content: the slice keeps it and the closers are written out after
	it, as the unfrozen element prints them.
-->

<div>
	<!-- prettier-ignore -->
	<pre>a
</div>

<ul>
	<!-- prettier-ignore -->
	<li><pre>a
</ul>
