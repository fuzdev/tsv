<!-- Block element containing {#if} expands to multiline -->
<div>{#if cond}text{/if}</div>

<!-- Block element containing {#each} expands to multiline -->
<p>{#each items as item}{item}{/each}</p>

<!-- Contrast: {#await} does NOT expand -->
<section>{#await promise}loading{/await}</section>
