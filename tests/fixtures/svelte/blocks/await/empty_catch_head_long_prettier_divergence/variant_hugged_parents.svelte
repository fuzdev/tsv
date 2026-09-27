<!-- an empty catch prints no body: {/await} stays hugged, one line at 100, head wraps at 101 -->
{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}{/await}

{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}{/await}

{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}{/await}

{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}{/await}

<!-- an identifier head wraps before its clause -->
{#await aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}{/await}

{#await aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}{/await}

<!-- a newline-authored body keeps the block form -->
{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}{/await}

<!-- a line comment forcing the pattern open breaks the enclosing block and element -->
{#if cond}{#await promise}{/await}{/if}

<span>text {#await promise}{/await} text</span>
