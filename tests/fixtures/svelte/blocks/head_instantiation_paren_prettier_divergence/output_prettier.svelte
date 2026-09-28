<script lang="ts"></script>

<!-- A block head followed by a word of the block's own keeps the pair around an instantiation
	expression: Svelte's parser hands the head to acorn-typescript, which reads the `as` / `then`
	after a bare `f<T>` as the continuation of a comparison. -->
{#each f<T> as item}{item}{/each}
{#each a.b<T> as item, i (i)}{item}{/each}
{#await f<T> then v}{v}{/await}

<!-- The pair wraps a head that ENDS on the close, whatever node ends it there. -->
{#each -f<T> as { a }}{a}{/each}
{#each a * f<T> as item}{item}{/each}
{#each a ?? f<T> as item}{item}{/each}
{#each <U>f<T> as item}{item}{/each}
{#each c ? a : f<T> as item}{item}{/each}
{#each () => f<T> as item}{item}{/each}
{#await -f<T> then v}{v}{/await}
{#await c ? a : f<T> then v}{v}{/await}
{#each a ?? b ?? f<T> as item}{item}{/each}
{#each a && b && f<T> as item}{item}{/each}
{#each a || b || f<T> as item}{item}{/each}
{#await a ?? b ?? f<T> then v}{v}{/await}
{#await a && b && f<T> then v}{v}{/await}
{#await a || b || f<T> then v}{v}{/await}

<!-- `catch` does not continue a comparison for acorn-typescript, but tsv's own parser refuses a
	bare `f<T>` ahead of it, so the pair keeps the head readable to tsv. -->
{#await f()<T> catch e}{e}{/await}
{#await f<T> catch}a{/await}

<!-- Inside `<pre>` the head prints on one line and takes the same pair. -->
<pre>{#each f<T> as item}{item}{/each}</pre>
<pre>{#await f<T> then v}{v}{/await}</pre>

<!-- Where the head's own `}` or `,` follows, nothing is joined to the close, and the pair strips
	in both formatters. -->
{#await f<T>}a{:then v}{v}{/await}
{#key f<T>}a{/key}
{#each f<T>}a{/each}
{#each f<T>, i}{i}{/each}
