<!-- the first wide tag travels and collapses flat; the second, in a hard-width run after it
     (plain-space boundary), stays welded to the line and breaks internally — both formatters
     keep that second form -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq {'rrrr' +
		'ssss'} tttt uuuu vvvv. {'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' +
		'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'} cccc
</p>

<!-- the traveled tag with a second, shorter tag packing flat in the continuation -->
<p>
	aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq {'rrrr' +
		'ssss'} tttt uuuu. {'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' + 'bbbbbbbbbbbbbbbbbbbb'} cccc dddd
</p>

<!-- short expressions fit inline, no travel (contrast) -->
<p>aaaa bbbb cccc {'dddd' + 'eeee'} ffff gggg {'hhhh' + 'iiii'} jjjj</p>
