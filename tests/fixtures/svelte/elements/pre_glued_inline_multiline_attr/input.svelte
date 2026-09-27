<!--
	Inside <pre> the content boundaries render, so an inline element glued to text on both sides
	whose own attribute forces the opening tag to break keeps its content on the tag's line and
	dangles its closing `>` onto the following text.
-->

<!-- a block-bodied handler -->
<pre>text1<button
		onclick={() => {
			a += 1;
			fn(a);
		}}>inline</button
	>text2</pre>

<!-- a quoted value spanning lines -->
<pre>text1<a
		href="/a"
		data-attr="value1
value2">inline1 inline2</a
	>text2</pre>
