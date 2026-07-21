<!--
	A then/catch shorthand with no name binding keeps the bare `then` / `catch` in the
	head — never demoted to {:then}/{:catch}. prettier normalizes an empty-pending full form
	to the shorthand.
-->

<!-- bare then-shorthand -->
{#await promise then}text{/await}

<!-- bare then-shorthand + catch with binding -->
{#await promise then}text{:catch error}text{/await}

<!-- bare then-shorthand + bare catch -->
{#await promise then}text{:catch}text{/await}

<!-- bare catch-shorthand -->
{#await promise catch}text{/await}

<!-- controls: shorthands with bindings (already correct) -->
{#await promise then value}text{/await}

{#await promise catch error}text{/await}

<!-- multiline (newline-authored path): bare then + catch -->
{#await promise then}
	<p>text</p>
{:catch error}
	<p>text</p>
{/await}
