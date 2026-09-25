<!--
	Glued inline-element pair (G2) whose RECEIVER holds non-text content: an expression tag, an
	element, a comment, or text and a tag. When the receiver's opening tag must wrap, tsv dangles
	the shedder's closing `>` onto the receiver's line exactly as it does for a text receiver, and
	the receiver lays out block-style. The receiver's content on its own lines, whether authored or
	printed, does not change the dangle; a receiver whose content is structurally multiline (a
	block) keeps its predecessor's `>` intact. Prettier wraps the receiver's attributes and keeps
	`</b><i` glued unless the shedder's own line is full — see output_prettier.svelte.
-->

<!-- 100: the opening-tag line fits, so no > dangles and only the content goes block-style -->
<b>a</b><i class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{b}</i
>

<!-- 101: the receiver's attributes wrap and the shedder's closing > dangles onto its line -->
<b>a</b><i class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{b}</i
>

<!-- an element child -->
<b>a</b><i class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	><u>b</u></i
>

<!-- a comment child -->
<b>a</b><i class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	><!-- c --></i
>

<!-- text and an expression tag -->
<b>a</b><i class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>a {b}</i
>

<!-- a receiver whose only authored line break is inside its content -->
<b>a</b><i class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{a}
	{b}</i
>

<!-- a force-broken attribute value wraps the receiver's opening tag at any width -->
<b>a</b><i
	title="x
y">{b}</i
>

<!-- control: a block child makes the receiver structurally multiline, so the > stays intact -->
<b>a</b><i class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{#if c}x{/if}</i
>

<!-- a component receiver -->
<b>a</b><Comp class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{b}</Comp
>

<!-- a component shedder -->
<Comp>a</Comp><i class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{b}</i
>

<!-- the receiver last in a run of three: only the pair whose receiver wraps dangles -->
<b>a</b><u>x</u><i class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{b}</i
>

<!-- in a block parent -->
<div>
	<b>a</b><i class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
		>{b}</i
	>
</div>

<!-- in an inline parent -->
<span
	><b>a</b><i
		class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">{b}</i
	></span
>

<!-- a wide shedder dangles its >; the receiver's own opening line is 101, so it wraps -->
<b class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"><u>b</u></b
><i class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{b}</i
>
