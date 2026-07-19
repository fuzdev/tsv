<!-- Single section: 100 chars - stays inline -->
{#await promise}<p>text text text text text text text text text text text text text txxx</p>{/await}

<!-- Single section: 101 chars - breaks to multiline -->
{#await promise}
	<p>text text text text text text text text text text text text text txxxx</p>
{/await}

<!-- Multi-section: 100 chars - stays inline -->
{#await promise}<p>text text text text textttxxxx</p>{:then value}<p>text text text text</p>{/await}

<!-- Multi-section: 101 chars - breaks all sections -->
{#await promise}
	<p>text text text text textttxxxxx</p>
{:then value}
	<p>text text text text</p>
{/await}

<!-- Multi-section with catch: 100 chars - stays inline -->
{#await promise}<p>text</p>{:then value}<p>text</p>{:catch error}<p>text text textxxxxxx</p>{/await}

<!-- Multi-section with catch: 101 chars - breaks all sections -->
{#await promise}
	<p>text</p>
{:then value}
	<p>text</p>
{:catch error}
	<p>text text textxxxxxxx</p>
{/await}

<!-- Then-shorthand: 100 chars - stays inline -->
{#await promise then value}<p>text text text text text text text text text text txxxxxxx</p>{/await}

<!-- Then-shorthand: 101 chars - breaks -->
{#await promise then value}
	<p>text text text text text text text text text text txxxxxxxx</p>
{/await}

<!-- Catch-shorthand: 100 chars - stays inline -->
{#await promise catch error}<p>text text text text text text text text text txxxxxxxxxxx</p>{/await}

<!-- Catch-shorthand: 101 chars - breaks -->
{#await promise catch error}
	<p>text text text text text text text text text txxxxxxxxxxxx</p>
{/await}
