<!-- fits at exactly 100: the element and the word after it both pack onto the text line -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo <code>pppp</code> qqqqq
	rrrr <code>ssss</code>
</p>

<!-- 101: only the word after the element wraps. The boundary before an inline element measures the
     ELEMENT alone — prettier's fill is pairwise (last word, separator, element), never the element
     plus the text that trails it — so the element stays on the text line at 94. -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo <code>pppp</code>
	qqqqqq rrrr <code>ssss</code>
</p>

<!-- The same boundary GLUED (no whitespace before the element), at exactly 100: the glued word and
     the element are one unit, and the word after it still packs onto the text line. -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn ooooo<code>pppp</code> qqqqq
	rrrr <code>ssss</code>
</p>

<!-- Glued at 101: the same pairwise answer as the space-authored case above. The glued unit ends at
     94, so it stays on the text line and only the word after it wraps — the run does NOT drop to a
     fresh line, since the overflow lands past the element rather than inside the pair. -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooooo<code>pppp</code>
	qqqqq rrrr <code>ssss</code>
</p>
