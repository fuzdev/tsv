<!--
	Inside `<pre>`, an escapable raw-text element (`<textarea>`, `<title>`) ends at an end tag the
	parser reads as an ordinary tag, whitespace before the `>` included, so its closing `>` dangles
	like any inline element's when the line is too wide.
-->

<!-- exactly 100 columns: nothing moves -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <textarea>text</textarea> text1 text2</pre>

<!-- 101 columns: the closing `>` dangles -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <textarea>text</textarea
	> text1 text2</pre>

<!-- exactly 100 columns: nothing moves -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <title>text</title> text1 text2</pre>

<!-- 101 columns: the closing `>` dangles -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <title>text</title
	> text1 text2</pre>
