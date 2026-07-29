<!--
	A short {#await} after a non-breakable sibling (plain text or a comment) stays
	fully inline — the parent element does not break. Await is inline-transparent
	after such a sibling, matching prettier. ({#snippet} is a declaration and takes
	its own line instead — blocks/snippet/own_line_prettier_divergence.)
-->

<!-- text sibling + await -->
<div>text {#await promise}loading{/await}</div>

<!-- comment sibling + await -->
<div><!-- c -->{#await promise}loading{/await}</div>
