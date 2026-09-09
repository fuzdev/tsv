<!-- A `//` inside a block head in a whitespace-sensitive element. The comment runs to end of
	line and so cannot share it, and the break lands inside the braced head where no character
	is content — so the head opens around the comment while never wrapping on width. -->

<!-- the comment-holder `(` the printer re-adds around a commented unary operand -->
<pre>{#if typeof ( // c
		ast) === 'object'}a{/if}</pre>

<pre>{#each !( // c
		cond) as item}{item}{/each}</pre>

<!-- the same holder inside an {#each} key, which prints a pair of its own -->
<pre>{#each items as item (!( // c
		cond))}{item}{/each}</pre>

<!-- a computed member's `[`, where the comment rides the bracket's own line -->
<pre>{#if obj[ // c
		key]}a{/if}</pre>

<pre>{#each obj[ // c
		key] as item}{item}{/each}</pre>

<!-- a template interpolation's `${`, where both formatters agree -->
<pre>{#if `x${a // c
	}y`}b{/if}</pre>

<!-- the OWN-LINE authoring at the same two delimiters: the comment keeps its own line, and
	the break above it is as much an obligation as the one below a glued comment — collapsed
	onto the delimiter, the next pass would read it as glued and re-space it -->
<pre>{#if typeof (
		// c
		ast) === 'object'}a{/if}</pre>

<pre>{#if obj[
		// c
		key]}a{/if}</pre>
