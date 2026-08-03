<!--
	A run holding MULTIPLE breakable expression tags reflows like any other fill: a welded
	word+tag unit that fits at its position stays flat, and the first unit that does not fit
	travels whole to the next line — no expression is torn open while a whitespace boundary
	could still absorb the overflow. Cases: at exactly 100 the whole run packs on one line;
	at 101 the last welded unit travels and collapses flat (the mid-run unit holds flat where
	it stands); glued ternary+tag pairs around a spaced `=` split at the boundary after the
	`=`, each pair intact; and a unit too wide even for a fresh line travels first, then
	breaks internally there.
-->
<p>
	{count1} word1{count1 !== 1 ? 's' : ''}, {count2abcdefghi} word2{count2abcdefghi !== 1 ? 's' : ''}
</p>
<p>
	{count1} word1{count1 !== 1 ? 's' : ''}, {count2abcdefghi}
	word2a{count2abcdefghi !== 1 ? 's' : ''}
</p>
<div>
	{cond1abcdefghijkl > 0 ? '+' : ''}{expr1abcdefgh} =
	{cond1abcdefghijkl > 0 ? '+' : ''}{expr2abcdefgh}%
</div>
<p>
	{count1} word1{count1 !== 1 ? 's' : ''}, {count3}
	word3{count3 !== 1
		? 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
		: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'}
</p>
