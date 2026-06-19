<!-- Single section: 100 chars - stays inline -->
{#await promise} <p>text text text text text text text text text text text text text tx</p> {/await}

<!-- Single section: 101 chars - breaks to multiline -->
{#await promise}
	<p>text text text text text text text text text text text text text txx</p>
{/await}

<!-- Multi-section: 100 chars - stays inline -->
{#await promise} <p>text text text text texttt</p> {:then value} <p>text text text text</p> {/await}

<!-- Multi-section: 101 chars - breaks all sections -->
{#await promise}
	<p>text text text text textttt</p>
{:then value}
	<p>text text text text</p>
{/await}

<!-- Multi-section with catch: 100 chars - stays inline -->
{#await promise} <p>text</p> {:then value} <p>text</p> {:catch error} <p>text text text</p> {/await}

<!-- Multi-section with catch: 101 chars - breaks all sections -->
{#await promise}
	<p>text</p>
{:then value}
	<p>text</p>
{:catch error}
	<p>text text textt</p>
{/await}

<!-- Then-shorthand: 100 chars - stays inline -->
{#await promise then value} <p>text text text text text text text text text text txxxxx</p> {/await}

<!-- Then-shorthand: 101 chars - breaks -->
{#await promise then value}
	<p>text text text text text text text text text text txxxxxx</p>
{/await}

<!-- Catch-shorthand: 100 chars - stays inline -->
{#await promise catch error} <p>text text text text text text text text text txxxxxxxxx</p> {/await}

<!-- Catch-shorthand: 101 chars - breaks -->
{#await promise catch error}
	<p>text text text text text text text text text txxxxxxxxxx</p>
{/await}
