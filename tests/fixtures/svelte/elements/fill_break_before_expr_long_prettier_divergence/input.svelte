<!-- Overflowing fill: tsv breaks before the {expression} tag to keep the line within printWidth,
     so the tag leads the continuation line with its trailing text (no leading space). -->
<p>
	{aaaaaaaaaaaaaaaaaaaa} bbbb cccc, {dddddddddddddddddddd} eeee ffff gggg ({hhhhhh} iiii),
	{jjjjjjjj} kkkk
</p>

<!-- One character shorter: the whole fill stays on a single block-style line (both formatters). -->
<p>
	{aaaaaaaaaaaaaaaaaaaa} bbbb cccc, {dddddddddddddddddddd} eeee ffff gggg ({hhhhhh} iii), {jjjjjjjj} kkkk
</p>
