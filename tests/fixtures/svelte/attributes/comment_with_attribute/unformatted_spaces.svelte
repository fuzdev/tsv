<!-- One real attribute is enough: every element kind keeps the comment, matching prettier -->
<div   data-attr="value"   /* c */   ></div>

<Comp   prop="value"   /* c */   />

<input   data-attr="value"   /* c */   />

<svelte:head   data-attr="value"   /* c */   ></svelte:head>

<slot   name="value"   /* c */   />

<!-- A mixed run in one gap: the block stays inline, the `//` forces the break -->
<div   data-attr="value"   /* c1 */   // c2
></div>
