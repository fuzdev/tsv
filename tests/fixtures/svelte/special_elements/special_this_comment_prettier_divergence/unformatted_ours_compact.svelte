<!-- Line comment before the synthesized `this`, no other attribute (prettier deletes it) -->
<svelte:element // c
this={x} />

<!-- Block comment before a plain-string `this` (prettier deletes it) -->
<svelte:element   /* c */   this="div"  ></svelte:element>

<!-- Line comment trailing `this`: stays trailing it (prettier deletes it) -->
<svelte:component this={Comp} // c
/>

<!-- Block comments on both sides of `this` keep their sides (prettier deletes both) -->
<svelte:element  /* c1 */  this={x}  /* c2 */  />

<!-- With a real attribute, a comment before `this` keeps that side -->
<!-- (prettier moves it after `this`, onto its own line) -->
<svelte:element // c
this={x} data-attr="value" />

<!-- With a real attribute, a comment trailing `this` stays trailing it -->
<!-- (prettier moves it to its own line) -->
<svelte:element this={x} // c
data-attr="value" />

<!-- Block comment before `this` with a real attribute (prettier moves it after `this`) -->
<svelte:element  /* c */  this={x}   data-attr="value"  />

<!-- A repeated `this` is an ordinary attribute, so only the first one is synthesized -->
<!-- and each comment still keeps its side (prettier moves the leading one past the binding) -->
<svelte:element  /* c1 */  this={a}  /* c2 */  this={b}  />

<!-- The component's synthesized `this` behaves the same way -->
<!-- (prettier moves the comment after `this`, onto its own line) -->
<svelte:component // c
this={Comp} prop="value" />

<!-- A comment after a plain-string `this` stays trailing it (prettier deletes it) -->
<svelte:element   this="div"   /* c */   />
