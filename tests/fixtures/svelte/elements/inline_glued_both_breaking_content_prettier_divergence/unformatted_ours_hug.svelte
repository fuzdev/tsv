<!--
	An inline element glued to text on both sides, whose content breaks however it is laid out,
	lays out block-style like any other inline element whose content goes multiline.
-->

<!-- a spaced sibling before a child whose block breaks -->
<div>
	text1<span>text2
	<strong>
		{#if cond}text3{/if}
	</strong></span>text4
</div>

<!-- the breaking child first, a spaced sibling after -->
<div>
	text1<span><strong>
		{#if cond}text2{/if}
	</strong> text3</span>text4
</div>

<!-- the breaking child alone -->
<div>
	text1<span><strong>
		{#if cond}text2{/if}
	</strong></span>text3
</div>

<!-- the breaking child glued to text on both sides inside the parent -->
<div>
	text1<span>
	text2<strong>
		{#if cond}text3{/if}
	</strong>text4</span>text5
</div>

<!-- a comment and a space before the breaking child -->
<div>
	text1<b><!-- c -->
	<strong>
		{#if cond}text2{/if}
	</strong>text3</b>text4
</div>

<!-- a component whose snippet holds the breaking child -->
<div>
	text1<Comp>text2{#snippet fn()}
		<strong>
			{#if cond}text3{/if}
		</strong>
	{/snippet}<i>inline1</i></Comp>text4
</div>

<!-- an await branch holding block elements -->
<div>
	text1<span>{#await promise}
		text2
	{:then value}
		<div>block1</div>
		<div>block2</div>
	{/await}</span>text3
</div>

<!-- a child whose attribute value spans lines, after a spaced sibling -->
<div>
	text1<span>text2
	<i
		data-attr="value1
value2"
	>
		inline1
	</i></span>text3
</div>

<!-- a child whose attribute value spans lines, alone -->
<div>
	text1<span><i
		data-attr="value1
value2"
	>
		inline1
	</i></span>text2
</div>

<!-- a multi-line comment in a child, between spaced siblings -->
<div>
	text1<span>text2
	<em>
		<!-- a
b -->
	</em> text3</span>text4
</div>

<!-- a multi-line comment between spaced siblings -->
<div>
	text1<span>text2 <!-- a
b --> text3</span>text4
</div>

<!-- a multi-line comment alone -->
<div>
	text1<span><!-- a
b --></span>text2
</div>

<!-- a frozen child whose bytes span lines, alone -->
<div>text1<span><!-- prettier-ignore --><i>
  inline1
</i></span>text2</div>

<!-- a frozen child whose bytes span lines, beside prose -->
<div>text1<span><!-- prettier-ignore --><i>
  inline2
</i> text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14
	text15</span>text16</div>
