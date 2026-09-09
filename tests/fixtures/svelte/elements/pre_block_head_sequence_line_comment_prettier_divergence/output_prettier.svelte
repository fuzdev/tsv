<!-- A `//` in a SEQUENCE's operand gap inside a block head in a whitespace-sensitive
	element. The comment ends its own line, so the next operand drops below it — inside the
	braced head, where no character is content. -->
<pre>{#if (a, b)}x{/if}</pre> // c

<pre>{#each (a, b) as item}{item}{/each}</pre> // c

<!-- the same gap inside an {#each} key, which prints a pair of its own -->
<pre>{#each items as item ((a, b))}{item}{/each}</pre> // c

<!-- a run of two, one per gap: each keeps its own line -->
<pre>{#if (a, b, c)}x{/if}</pre> // c // c2

<!-- the sequence nested in the comment-holder `(` of a unary operand -->
<pre>{#if !(a, b)}x{/if}</pre> // c
