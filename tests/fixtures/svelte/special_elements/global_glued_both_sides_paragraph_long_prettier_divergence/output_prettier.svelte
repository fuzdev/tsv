<!--
	A global `svelte:*` element glued to content on both sides, inside a paragraph: it flows with
	the paragraph as the glued inline element it lays out as does.
-->

<!-- an authored newline beside the glued unit reflows with the prose -->
text1 text2<svelte:window on:x={fn1} />text3 <b>text4</b> text5 text6 text7 text8

<!-- the glued unit would end its line at 101: it travels to a fresh line whole -->
text9 text10 text11 text12 text13 text14 text15<svelte:head><title>text16</title></svelte:head
>text17

<!-- the glued unit ends its line at exactly 100: it stays -->
text18 text19 text20 text21 text22 text23 text24 text25 text26<svelte:document on:xxx={fn2} />text27

<!-- the glued unit would end its line at 101: it travels to a fresh line whole -->
text28 text29 text30 text31 text32 text33 text34 text35 text36<svelte:body
	on:resize={fn300}
/>text37
