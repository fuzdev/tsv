<!-- A `//` in a SEQUENCE's operand gap inside a block head in a whitespace-sensitive
	element. The comment ends its own line, so the next operand drops below it — inside the
	braced head, where no character is content. -->
<pre>{#if (a, // c
	b)}x{/if}</pre>

<pre>{#each (a, // c
	b) as item}{item}{/each}</pre>

<!-- the same gap inside an {#each} key, which prints a pair of its own -->
<pre>{#each items as item ((a, // c
	b))}{item}{/each}</pre>

<!-- a run of two, one per gap: each keeps its own line -->
<pre>{#if (a, // c
	b, // c2
	c)}x{/if}</pre>

<!-- the sequence nested in the comment-holder `(` of a unary operand -->
<pre>{#if !(a, // c
	b)}x{/if}</pre>
