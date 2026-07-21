<!--
	A short {#await}/{#snippet} after a non-breakable sibling (plain text or a
	comment) stays fully inline — the parent element does not break. await/snippet
	are inline-transparent after such a sibling, matching prettier.
-->

<!-- text sibling + await -->
<div>text {#await promise}loading{/await}</div>

<!-- comment sibling + await -->
<div><!-- c -->{#await promise}loading{/await}</div>

<!-- text sibling + snippet -->
<div>text {#snippet fn()}content{/snippet}</div>

<!-- comment sibling + snippet -->
<div><!-- c -->{#snippet fn()}content{/snippet}</div>
