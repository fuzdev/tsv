<!-- expression breaks to multiline, text continues on continuation line -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq {'rrrr' +
		'ssss'} tttt uuuu vvvv wwww xxxx
</p>

<!-- expression breaks, text and second expression on continuation line -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq {'rrrr' +
		'ssss'} tttt uuuu {'vvvv' + 'wwww'} xxxx yyyy zzzz
</p>

<!-- expression doesn't break, all inline -->
<p>aaaa bbbb {'cccc' + 'dddd'} eeee ffff gggg</p>
