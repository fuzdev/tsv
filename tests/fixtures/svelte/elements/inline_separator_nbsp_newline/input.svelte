<!--
	"Did the author leave this run on ONE line?" is a property of the WHOLE run, and the newline
	that answers it can ride in a separator node that is not whitespace-only: `&nbsp;` decodes to
	a Unicode space, so the node is template CONTENT to the whitespace scalars while still being
	a separator (it carries no word for a fill to pack). The run below therefore holds a spaced
	pair AND an authored break, and a run of pure siblings with no prose keeps its authored lines
	— so it is its own fixed point, exactly like the `&nbsp;`-free control beneath it. Reading
	only the whitespace-ONLY nodes let the one spaced pair speak for the whole run and flattened
	it onto a single line, deleting a break the author wrote.
-->
<div>
	<span>
		<code>a</code> <code>b</code>&nbsp;
		<code>c</code>
	</span>
</div>

<div>
	<span>
		<code>a</code> <code>b</code>
		<code>c</code>
	</span>
</div>
