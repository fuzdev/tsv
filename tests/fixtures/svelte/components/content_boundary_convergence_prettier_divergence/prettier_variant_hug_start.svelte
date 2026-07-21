<!--
	One document, three authorings of the content boundary. The whitespace between a tag and its
	content is render-free under Svelte 5, so it must not select the layout: tsv converges all
	three on the block-style form below (both tags intact, content on its own indented line).
	Prettier keeps a different stable form for each — see the prettier_variant_* files.
-->
<Comp
	prop1="value1 value2"
	prop2="value3 value4"
	prop3="value5 value6"
	prop4="value7 value8"
	prop5="value9 value10"
	><div>x</div>
</Comp>
