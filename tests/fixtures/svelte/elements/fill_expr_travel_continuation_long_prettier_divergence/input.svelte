<!-- wide expression tag travels to the continuation line and collapses flat; text continues after it -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq
	{'rrrr' + 'ssss'} tttt uuuu vvvv wwww xxxx
</p>

<!-- traveled tag with text and a second expression tag flowing on the continuation line -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq
	{'rrrr' + 'ssss'} tttt uuuu {'vvvv' + 'wwww'} xxxx yyyy zzzz
</p>

<!-- short expression fits inline, no travel -->
<p>aaaa bbbb {'cccc' + 'dddd'} eeee ffff gggg</p>
