<!--
	A WELDED unit — two inline elements fused by glued text — is one fill item: no break may land
	inside it, since breaking at a glued boundary would inject a rendered space. The boundary in
	front of it therefore measures the WHOLE unit. At exactly 100 the unit packs onto the text line
	and only the tail after it wraps.
-->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk lllllll <code>pppp</code>/<code>qqqq</code>
	rrrr ssss
</p>

<!--
	101: the unit does not fit after the last word, so the WHOLE unit drops to a fresh line together
	— the `/` boundary stays glued and the tail packs after the unit.
-->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llllllll
	<code>pppp</code>/<code>qqqq</code> rrrr ssss
</p>
