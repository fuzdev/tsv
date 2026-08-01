<!-- An own-line comment in a whitespace-sensitive element's head carries its line break with
	it: the attributes wrap one per line and the `>` still hugs the last one, so no character
	is added to the literal content -->
<textarea
	data-attr1="value1"
	/* c */
	data-attr2="value2">text</textarea
>

<!-- The same head shape on an inline element inside `<pre>` -->
<pre><span
		data-attr1="value1"
		/* c */
		data-attr2="value2">inline1</span
	></pre>
