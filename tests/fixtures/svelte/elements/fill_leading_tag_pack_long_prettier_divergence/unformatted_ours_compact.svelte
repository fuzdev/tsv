<!--
	Text packing after a LEADING tag holds printWidth. The fill after a `{expr}` tag leads
	with a collapsible `line`, and tsv measures that separator TOGETHER with the word it
	stands before, so a word packs onto the tag's line only while the line stays <= 100.
	Prettier packs one word past the limit at these widths — see output_prettier.svelte.
	Cases at the exact boundary of the pack decision: at 100 (tag line 94, the first word
	lands exactly at 100) tsv packs the word where prettier packs a second (106); at 101
	(tag line 95, the first word would land at 101) tsv breaks the leading line and packs
	nothing where prettier still packs the word (101). Contrast fill_leading_line, where
	the tag line sits at exactly 100 and both formatters break the leading line.
-->
<li>{xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx} ccccc ccccc ccccc</li>
<li>{xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx} ccccc ccccc ccccc</li>
