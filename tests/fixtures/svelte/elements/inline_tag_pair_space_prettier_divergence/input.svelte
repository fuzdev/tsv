<!--
	A space-spelled separator between two tags is a fill boundary that packs per width, whatever
	its run holds — no word at all, one word, or prose — where prettier's bare `line` between two
	tags breaks with the multiline container. The sibling-newline flow rule's prose gate is a
	HOLD on an authored newline and never turns a space into one: the newline spelling of the
	same pair is held (the control), so the two spellings are two fixed points.
-->

<!-- a wordless pair: the run holds no text at all, and the pair still packs -->
<p>
	{expr1} {expr2}
</p>

<!-- a word before a spaced tag pair, and a word after one -->
<p>
	text1 {expr1} {expr2}
</p>
<p>
	{expr1} {expr2} {expr3} text1
</p>

<!-- at the root, which is always a multiline container: the escaped-angle-bracket pair -->
{'<'}div{'>'} {'<'}code{'>'}{'<'}/code{'>'}

<!-- an inline element or a component before a tag, and a component between two tags, pack the
     same way: one answer at every sibling kind, where a tag before an element or component
     already packed -->
<p>
	<span>inline1</span> {expr}
</p>
<p>
	<Comp /> {expr}
</p>
<p>
	{expr1} <Comp /> {expr2}
</p>

<!-- every tag kind in one run -->
<p>
	{@html a} {@render fn()} {expr}
</p>

<!-- control: the newline spelling of the wordless pair is held — the rule holds a newline, it
     never forces one -->
<p>
	{expr1}
	{expr2}
</p>
