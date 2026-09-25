<!--
	A global `svelte:*` element glued to content on ONE side only takes its own line: the compiler
	hoists it out of the fragment, so the break on its glued side merges with the whitespace (or
	the fragment edge) on its other side and renders nothing new.
-->
<svelte:document on:x={fn1} />text1<svelte:window on:x={fn2} /> text2<svelte:body on:x={fn3} />
