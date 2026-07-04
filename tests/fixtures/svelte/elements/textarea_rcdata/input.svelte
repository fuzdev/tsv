<!-- RCDATA: nested tags are literal text, not elements; {expr} still interpolates -->
<textarea>
	<p>text1 {expr}</p>
</textarea>

<!-- Raw angle brackets stay literal text -->
<textarea><span>text2</textarea>

<!-- Expression-only content (no surrounding text nodes) -->
<textarea>{value}</textarea>
