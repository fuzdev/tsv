<!-- A same-line `//` between two attributes of a whitespace-sensitive element: the comment
	stays trailing the token it was written after while the list wraps around it, and the
	`>` still hugs the last attribute so no character is added to the literal content -->
<textarea data-attr1="value1" // c
data-attr2="value2">text</textarea>

<pre><span data-attr1="value1" // c
data-attr2="value2">inline1</span></pre>
