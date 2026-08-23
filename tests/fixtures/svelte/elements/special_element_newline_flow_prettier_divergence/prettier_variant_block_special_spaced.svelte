<!--
	The sibling-newline flow rule at a `svelte:*` SPECIAL element — the one sibling kind the rule's
	own fixtures never reach (theirs are HTML elements, components and tags). The rule asks each
	neighbour one question, "does it flow?", and a special element answers it by the same
	block/inline classification a plain element does: the inline specials flow like a `<span>`, and
	the four global ones (`svelte:head` / `svelte:window` / `svelte:body` / `svelte:document`) are
	block-classified and own their line. Both halves are needed — without the block one, "a special
	element flows" is equally satisfied by a rule that never asks the classification at all. The
	block half is root-only, because Svelte rejects those four inside an element or a block.
-->
<p>
	text1 <svelte:element this={tag}>inline1</svelte:element> text2 text3
</p>
<p>
	text1 <slot name="value" /> text2 text3
</p>
<p>
	text1 <svelte:boundary>inline1</svelte:boundary> text2 text3
</p>

<!-- control: a BLOCK svelte:* element owns its line, so the run does not flow through it -->
text1 <svelte:window /> text2 text3
