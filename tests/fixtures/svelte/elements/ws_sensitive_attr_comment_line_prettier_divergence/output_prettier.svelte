<!-- A `//` in a whitespace-sensitive element's attribute list runs to end of line, so the `>`
	takes its own line instead of hugging the last attribute. The break lands inside the tag,
	where no character is content, so the literal text is untouched. -->

<!-- With content, the attributes keep the tag-name line and the `>` hugs the content one
	level in, the layout an over-width `<textarea>` already takes -->
<pre>text</pre>

<pre
	data-attr="value">text</pre> // c

<!-- With no content there is nothing for the `>` to protect, so it sits at base indent -->
<pre></pre>

<pre
	data-attr="value"></pre> // c

<!-- `<textarea>` is the same element class and takes the same two shapes -->
<textarea>text</textarea>

<textarea
	data-attr="value"></textarea> // c
