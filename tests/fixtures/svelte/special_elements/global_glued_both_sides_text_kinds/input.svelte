<!-- non-collapsible and multibyte text glued on both sides keeps the element on its line -->

<!-- an entity-encoded non-breaking space on both sides -->
text1&nbsp;<svelte:window on:x={fn1} />&nbsp;text2

<!-- multibyte characters on both sides -->
é<svelte:document on:x={fn2} />漢
