<!-- the FIRST `this` becomes the element's tag; every later one stays an ordinary attribute -->
<svelte:element this={a} this={b}>text</svelte:element>

<!-- string spelling -->
<svelte:element this="div" this="span">text</svelte:element>

<!-- a later `this` need not be usable as a tag: bare, and multi-chunk -->
<svelte:element this={a} this>text</svelte:element>
<svelte:element this={a} this="a{b}">text</svelte:element>

<!-- the tag hoists past the attributes written before it -->
<svelte:element this={a} data-attr1="value1" data-attr2="value2" this={b}>text</svelte:element>

<!-- svelte:component takes the same branch; only the first `this` must be an {expression} -->
<svelte:component this={Comp} this="value" />
<svelte:component this={Comp} this={b} />
