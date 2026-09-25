<!--
	A global `svelte:*` element glued to text on both sides, in a line too wide to fit: it lays out
	as an inline element glued on both sides does, and neither glued boundary takes a break.
-->

<!--
	with content, one character too wide for the closing-`>` dangle (the dangle line would be
	101): the content goes block-style between the two intact tags
-->
text1<svelte:head><title>text2</title><meta name="value1" content="value2value2value" /></svelte:head>text3

<!-- inside a paragraph: the welded unit moves to a fresh line as one -->
text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14 text15 text16 text17 text18<svelte:window on:resize={fn1} />text19 text20 text21 text22 text23 text24 text25 text26

<!-- inside a paragraph, the glued unit would end the line at 101: it moves to a fresh line -->
text27 text28 text29 text30 text31 text32 text33 text34 text35<svelte:body on:resize={fn100} />text36 text37 text38
