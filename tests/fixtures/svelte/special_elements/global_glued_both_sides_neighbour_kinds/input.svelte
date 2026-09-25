<!--
	The glue is asked of the nearest CONTENT on each side. A hoisted neighbour is not content, so a
	run of global `svelte:*` elements is glued to the text at both of its ends; a component is
	content; and a comment counts as content, as it does for declaration tags. Each element stays
	on its neighbours' line.
-->

<!-- a run of two, text on both ends of the run -->
text1<svelte:window on:resize={fn1} /><svelte:body on:mouseenter={fn2} />text2

<!-- components on both sides -->
<Comp1 /><svelte:document on:visibilitychange={fn3} /><Comp2 />

<!-- a comment on one side, text beyond it -->
text3<svelte:head><title>text4</title></svelte:head><!-- comment -->text5
