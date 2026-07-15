<svelte:element this={/* c */ tag}></svelte:element>
<svelte:element this={/* a */ /* b */ tag}></svelte:element>
<svelte:element this={/* c */ 'a' + 'b'}></svelte:element>
<svelte:element this={/* c */ `hello`}></svelte:element>
<svelte:element this={a ? /* c */ 'a' : 'b'}></svelte:element>
<svelte:element this={fn(/* c */ a)}></svelte:element>
<svelte:element this={a && /* c */ b}></svelte:element>
<svelte:component this={/* c */ Comp}></svelte:component>
<svelte:element
	this={// c
	tag}
></svelte:element>
<svelte:component
	this={// c
	Comp}
></svelte:component>
