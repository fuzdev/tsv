<!-- middle text after breaking expression continues on continuation line -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq {'rrrr' +
		'ssss'} tttt uuuu vvvv {'wwww' + 'xxxx'} yyyy
</p>

<!-- middle text with single word after breaking expression -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq {'rrrr' +
		'ssss'} tttt {'uuuu' + 'vvvv'} wwww
</p>

<!-- expression doesn't break, all inline (contrast) -->
<p>aaaa bbbb {'cccc' + 'dddd'} eeee {'ffff' + 'gggg'} hhhh</p>
