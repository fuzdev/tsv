<!-- Each delimiter of a whitespace-sensitive element reads its OWN content edge: the
	opening `>` takes a break point of its own only when the content opens on a visible
	byte, and the closing `>` dangles only when the content ends on one -->

<!-- both edges visible: the `>` hugs the last attribute, the closing `>` dangles -->
<textarea
	data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb">text1</textarea
>

<!-- opening edge is whitespace: the `>` stays hugged, the closing `>` still dangles -->
<textarea
	data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"> text2</textarea
>

<!-- closing edge is whitespace: the `>` takes its own line, the closing tag stays whole -->
<textarea
	data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	>text3 </textarea>

<!-- both edges whitespace: neither delimiter moves -->
<textarea
	data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"> text4 </textarea>

<!-- the same four on an inline element inside `<pre>` -->
<pre><span
		data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb">inline1</span
	></pre>
<pre><span
		data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"> inline2</span
	></pre>
<pre><span
		data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
		>inline3 </span></pre>
<pre><span
		data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"> inline4 </span></pre>

<!-- control: a block element hugs the `>` and keeps its closing tag whole either way -->
<pre
	data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb">text5</pre>
<pre
	data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb">text6 </pre>

<!-- the same two edges on multiline content, where the `>` break is forced anyway:
	only the closing delimiter is still at stake -->
<pre><span data-attr="value"
		>line1
line2</span
	></pre>
<pre><span data-attr="value"
		>line1
line2 </span></pre>
<pre><span data-attr="value"
		>line1
line2
</span></pre>

<!-- the closing edge is the LAST CHILD, not the last text node -->
<pre><span data-attr="value"
		>line1
{expr}</span
	></pre>
