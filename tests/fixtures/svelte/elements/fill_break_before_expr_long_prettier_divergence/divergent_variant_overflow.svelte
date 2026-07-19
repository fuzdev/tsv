<!-- Overflowing fill: tsv breaks before the {expression} tag to keep the line within printWidth,
     so the tag leads the continuation line with its trailing text (no leading space). -->
<p>
	{aaaaaaaaaaaaaaaaaaaa} bbbb cccc, {dddddddddddddddddddd} eeee ffff gggg ({hhhhhh} iiii), {jjjjjjjj}
	kkkk
</p>

<!-- One character shorter: prettier keeps the whole fill on one line (105), tsv breaks the
     trailing word off to hold the line at exactly 100. -->
<p>
	{aaaaaaaaaaaaaaaaaaaa} bbbb cccc, {dddddddddddddddddddd} eeee ffff gggg ({hhhhhh} iii), {jjjjjjjj} kkkk
</p>
