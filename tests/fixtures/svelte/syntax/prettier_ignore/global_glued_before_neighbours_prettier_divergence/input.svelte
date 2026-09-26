<!--
	Whatever the directive is glued to, the whitespace after the frozen global element is a line
	break, as it is after the element unfrozen.
-->

<!-- after a block element, the directive starts its line -->
<p>block1</p>
<!-- prettier-ignore --><svelte:window    on:x={fn1}   />
text1

<!-- a component before -->
<Comp /><!-- prettier-ignore --><svelte:document    on:x={fn2}   />
text2

<!-- a space before the directive -->
text3 <!-- prettier-ignore --><svelte:head>  <title>text4</title>  </svelte:head>
text5

<!-- control: a range around the element -->
text6<!-- prettier-ignore-start --><svelte:body /><!-- prettier-ignore-end -->
text7
