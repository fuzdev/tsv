<!--
	A glued inline-element pair (G2) whose SHEDDER, the first element, is too wide for its line.
	Its content drops to its own indented line and so does its closing tag, exactly as for an
	element that keeps its >: the shedder's > then hugs the receiver's opening tag when that fits
	and dangles onto the receiver's line when the receiver's attributes wrap. A shedder multiline
	only by authored line breaks sheds its > the same way; one multiline by structure (a block
	child) keeps it. Prettier prints each block-style shedder the same, but joins </b><i where tsv
	dangles the > and hugs a component shedder's content — see output_prettier.svelte.
-->

<!-- 100: the shedder's line to the receiver's opening tag fits, so it stays flat -->
<b class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">x</b><i>
	x
</i>

<!-- 101: the shedder's content drops to its own line and so does its closing tag -->
<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
	x</b
><i>x</i>

<!-- an expression tag in the shedder -->
<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
	{y}</b
><i>{y}</i>

<!-- an element in the shedder -->
<b class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
	<u>y</u></b
><i>x</i>

<!-- a comment in the shedder -->
<b class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
	<!-- c --></b
><i>x</i>

<!-- a component shedder -->
<Comp class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">x</Comp><i
	>x</i
>

<!-- the receiver's attributes wrap too: the > dangles from the shedder's closing-tag line -->
<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
	x</b
><i
	class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
>
	x
</i>

<!-- the middle of a run of three -->
<u>a</u><b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
	x</b
><i>x</i>

<!-- the middle of a run of three receives a > and sheds its own -->
<b>a</b><i
	class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
>
	{y}</i
><u>
	{y}
</u>

<!-- behind a glued comment -->
<!-- c --><b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
	x</b
><i>x</i>

<!-- after prose: the pair travels to a fresh line and the shedder goes block-style there -->
<p>
	some text
	<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
		x</b
	><i>x</i> more
</p>

<!-- a shedder multiline only by authored line breaks sheds its > like its width-broken twin -->
<b>
	{y}
</b><i
	class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
>
	x
</i>

<!-- control: a shedder multiline by structure (a block child) keeps its > intact -->
<a href="/">
	{#if x}a{/if}
</a><i
	class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
>
	x
</i>

<!-- in a block parent -->
<div>
	<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
		x</b
	><i>x</i>
</div>

<!-- in an inline parent -->
<span>
	<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
		x</b
	><i>x</i>
</span>
