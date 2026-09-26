<!-- A comment that breaks a block binding pattern inside a whitespace-sensitive element: the
	pattern opens around it inside the braced head, where no character is content, and every
	section stays glued to its tags -->

<!-- {#each}: the context pattern -->
<pre>{#each items as {
		a
		// c
	}}{a}{/each}</pre>

<!-- {#await}: the then-shorthand and the catch-shorthand pattern -->
<pre>{#await promise then {
		a
		// c
	}}{a}{/await}</pre>

<pre>{#await promise catch {
		e
		/* c */
	}}{e}{/await}</pre>

<!-- {#await}: the {:then} and {:catch} patterns, after a then-shorthand with its own binding -->
<pre>{#await promise}loading{:then {
		a
		// c
	}}{a}{:catch error}{error}{/await}</pre>

<pre>{#await promise then value}{value}{:catch {
		e
		/* c */
	}}{e}{/await}</pre>

<!-- {#snippet}: a parameter pattern -->
<pre>{#snippet fn({
		a
		// c
	})}{a}{/snippet}</pre>
