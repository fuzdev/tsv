<!-- middle text before breaking expression continues on continuation line -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq {'rrrr' +
		'ssss'} tttt uuuu vvvv. {'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' +
		'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'} cccc
</p>

<!-- two middle text segments surrounding expression, both with ws -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq {'rrrr' +
		'ssss'} tttt uuuu. {'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' + 'bbbbbbbbbbbbbbbbbbbb'} cccc dddd
</p>

<!-- short expression doesn't break, all inline (contrast) -->
<p>aaaa bbbb cccc {'dddd' + 'eeee'} ffff gggg {'hhhh' + 'iiii'} jjjj</p>
