<!-- wide expression tag travels and collapses flat; middle text and a second tag flow after it -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq
	{'rrrr' + 'ssss'} tttt uuuu vvvv {'wwww' + 'xxxx'} yyyy
</p>

<!-- single word between the traveled tag and a second tag -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq
	{'rrrr' + 'ssss'} tttt {'uuuu' + 'vvvv'} wwww
</p>

<!-- short expressions fit inline, no travel (contrast) -->
<p>aaaa bbbb {'cccc' + 'dddd'} eeee {'ffff' + 'gggg'} hhhh</p>
