<!-- a global `svelte:*` element glued to text on both sides keeps the glue at every width -->

<!-- 100 wide: stays on one line -->
text1<svelte:window on:resize={fn1} on:scroll={fn2} on:focus={fn3} on:blur={fn4} on:xx={fn5} />text2

<!-- 101 wide: the attributes break, the tag stays glued to the text on both sides -->
text3<svelte:document
	on:visibilitychange={fn6}
	on:x={fn7}
	on:y={fn8}
	on:z={fn9}
	on:ww={fn10}
/>text4

<!-- with content, 100 wide: stays on one line -->
text5<svelte:head><title>text6</title><meta name="value1" content="value2valu" /></svelte:head>text7

<!-- inside a paragraph, the glued unit ends the line at exactly 100: it stays on the line -->
text10 text11 text12 text13 text14 text15 text16 text17 text18<svelte:body on:resize={fn10} />text19
text20 text21
