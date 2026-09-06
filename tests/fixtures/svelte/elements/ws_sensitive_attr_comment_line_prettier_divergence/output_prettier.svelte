<!-- A `//` in a whitespace-sensitive element's attribute list runs to end of line, so the `>`
	takes its own line instead of hugging the last attribute. The break lands inside the tag,
	where no character is content, so the literal text is untouched. -->

<!-- With content, the `>` takes the next line one level in and hugs the content there, the
	layout an over-width `<textarea>` already takes. The list wraps around the `//` like any
	other list holding one, so only a comment the author put on the tag-name line stays -->
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

<!-- An own-line `//` before the `>` carries its line break with it: the attributes wrap
	one per line and the `>` keeps the next line, one level in -->
<pre
	data-attr="value">text</pre>
	// c
