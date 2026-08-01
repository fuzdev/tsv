<!-- Line comment after the tag name, no attributes: stays there (prettier deletes it) -->
<div></div>

<!-- Own-line comment in an empty attribute list: keeps its own line (prettier deletes it) -->
<div></div>

<!-- Inline block comment, no attributes: stays inline (prettier deletes it) -->
<div></div>

<!-- Own-line block comment: keeps its own line (prettier deletes it) -->
<div></div>

<!-- Consecutive block comments keep their order (prettier deletes both) -->
<div></div>

<!-- Element with content: the comment is in the open tag (prettier deletes it) -->
<div>text</div>

<!-- A `//` breaks the open tag, so the content goes block-style with it -->
<div>text</div>

<!-- Self-closing component (prettier deletes it) -->
<Comp />

<!-- Line comment on a self-closing component: `/>` drops to the next line -->
<Comp />

<!-- Void element (prettier deletes it) -->
<input />

<!-- Special element with no `this` binding (prettier deletes it) -->
<svelte:head></svelte:head>

<!-- Special element, self-closing, line comment (prettier deletes it) -->
<svelte:window />

<!-- Slot (prettier deletes it) -->
<slot />

<!-- A mixed run in one gap: the block stays inline, the `//` still forces the break (prettier deletes both) -->
<div></div>

<!-- Whitespace-sensitive elements (their own printer) keep the comment too (prettier deletes it) -->
<pre>text</pre>

<textarea></textarea>
