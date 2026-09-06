<!-- A whitespace-sensitive head whose attribute list must break wraps one attribute per line
	and the `>` hugs the last one, so no character is added to the literal content. Here a
	newline inside the value is what forces the break. -->
<textarea data-attr1="value1,
	value2" data-attr2="value2">text</textarea>

<!-- The same head on an inline element inside `<pre>` -->
<pre><span data-attr1="value1,
	value2" data-attr2="value2">inline1</span></pre>

<!-- A block element answers the same way, and keeps its closing tag intact -->
<pre data-attr1="value1,
	value2" data-attr2="value2">text</pre>
