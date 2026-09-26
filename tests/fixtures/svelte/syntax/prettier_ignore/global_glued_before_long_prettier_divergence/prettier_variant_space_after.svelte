<!--
	A frozen global `svelte:*` element glued to the text before it ends that text's line: the break
	after it is not part of the glued unit's width.
-->

<!-- the glued unit ends its line at exactly 100: it stays -->
text1 text2 text3 text4 text5 text6 text7 text8<!-- prettier-ignore --><svelte:window on:xx={fn1} /> text9

<!-- the glued unit would end its line at 101: it travels to a fresh line whole -->
text10 text11 text12 text13 text14 text15<!-- prettier-ignore --><svelte:document on:xxxxxxx={fn2} />
text16
