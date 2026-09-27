<!-- an empty catch prints no body: {/await} stays hugged, one line at 100, head wraps at 101 -->
{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) then value} {:catch} {/await}

{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) then value} {:catch} {/await}

{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) then value} {:catch error} {/await}

{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) then value} {:catch error} {/await}

<!-- an identifier head wraps before its clause -->
{#await aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa then value} {:catch} {/await}

{#await aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa then value} {:catch} {/await}

<!-- a newline-authored body keeps the block form -->
{#await fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) then value} {:catch} 
{/await}

<!-- a line comment forcing the pattern open breaks the enclosing block and element -->
{#if cond}{#await promise then value} {:catch {
	a, // c
	b
}} {/await}{/if}

<span>text {#await promise then value} {:catch {
	a, // c
	b
}} {/await} text</span>
