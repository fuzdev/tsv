<!-- an empty catch prints no body: {/await} stays hugged, one line at 100, head wraps at 101 -->
{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) catch} {/await}

{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) catch} {/await}

{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) catch error} {/await}

{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) catch error} {/await}

<!-- an identifier head wraps before its clause -->
{#await aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa catch} {/await}

{#await aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa catch} {/await}

<!-- a newline-authored body keeps the block form -->
{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) catch} 
{/await}

<!-- a line comment forcing the pattern open breaks the enclosing block and element -->
{#if cond}{#await promise catch {
	a, // c
	b
}} {/await}{/if}

<span>text {#await promise catch {
	a, // c
	b
}} {/await} text</span>
