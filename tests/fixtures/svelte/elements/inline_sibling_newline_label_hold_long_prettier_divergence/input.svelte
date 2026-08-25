<!-- 100 chars - fits: the space spelling of a one-word run stays inline -->
<p>
	<Comp prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" /> text1
</p>

<!-- 101 chars - overflows: the fill wraps at the separator, onto the held (newline) spelling -->
<p>
	<Comp prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" />
	text1
</p>

<!-- the same boundary at a void element: 100 fits -->
<label>
	<input type="checkbox" prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" /> text1
</label>

<!-- 101 wraps -->
<label>
	<input type="checkbox" prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" />
	text1
</label>

<!-- the tag alone overflows: its attributes break, and the word stays hugged to the closing tag -->
<p>
	<Comp
		prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	/> text1
</p>
