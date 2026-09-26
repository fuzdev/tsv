<!--
	A frozen INLINE node keeps the space after it, as it does unfrozen: an element, a component, an
	expression tag, a control-flow block, a `svelte:element`.
-->

<!-- an element -->
text1<!-- prettier-ignore --><span    data-attr="value"   >inline1</span> text2

<!-- a component -->
text3<!-- prettier-ignore --><Comp    prop="value"   /> text4

<!-- an expression tag -->
text5<!-- prettier-ignore -->{   expr   } text6

<!-- a control-flow block -->
text7<!-- prettier-ignore -->{#if   cond}text8{/if} text9

<!-- a svelte:element -->
text10<!-- prettier-ignore --><svelte:element  this={tag} >text11</svelte:element> text12

<!-- control: a no-break space after a frozen global element is content, not a separator -->
text13<!-- prettier-ignore --><svelte:window    on:x={fn1}   /> text14
