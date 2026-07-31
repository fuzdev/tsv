<!-- Line comment after the tag name, no attributes: stays there (prettier deletes it) -->
<div // c
></div>

<!-- Own-line comment in an empty attribute list: keeps its own line (prettier deletes it) -->
<div
// c
></div>

<!-- Inline block comment, no attributes: stays inline (prettier deletes it) -->
<div    /* c */   ></div>

<!-- Own-line block comment: keeps its own line (prettier deletes it) -->
<div
    /* c */
></div>

<!-- Consecutive block comments keep their order (prettier deletes both) -->
<div   /* c1 */   /* c2 */  ></div>

<!-- Element with content: the comment is in the open tag (prettier deletes it) -->
<div  /* c */  >text</div>

<!-- A `//` breaks the open tag, so the content goes block-style with it -->
<div // c
>text</div>

<!-- Self-closing component (prettier deletes it) -->
<Comp   /* c */   />

<!-- Line comment on a self-closing component: `/>` drops to the next line -->
<Comp // c
/>

<!-- Void element (prettier deletes it) -->
<input  /* c */  />

<!-- Special element with no `this` binding (prettier deletes it) -->
<svelte:head  /* c */  ></svelte:head>

<!-- Special element, self-closing, line comment (prettier deletes it) -->
<svelte:window // c
/>

<!-- Slot (prettier deletes it) -->
<slot   /* c */   />

<!-- A mixed run in one gap: the block stays inline, the `//` still forces the break (prettier deletes both) -->
<div/* c1 */// c2
></div>
