<!-- the value's whitespace-only run re-indents, and its break wraps the head -->
<pre
	style:transform-origin="{expr1}
	{expr2}">text</pre>

<textarea
	style:transform-origin="{expr1}
	{expr2}"></textarea>

<!-- contrast: a plain attribute value is opaque here too, and both wrap the head for it -->
<pre
	data-attr="a,
      b">text</pre>

<!-- an ANCESTOR is enough: the run re-indents on a nested element too -->
<pre><span
		style:transform-origin="{expr1}
		{expr2}">text</span
	></pre>
