<!-- A block head inside a whitespace-sensitive element never wraps on width: a line break
	added there would be rendered content. Both formatters agree, and the 100/101 boundary is
	inert at all three head sites the whitespace-sensitive builder owns — each head has a break
	point an ordinary head would take at 101, and none of them takes it here. -->

<!-- {#if} test, 100 chars -->
<pre>{#if aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}text{/if}</pre>

<!-- 101 chars - still inline -->
<pre>{#if aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}text{/if}</pre>

<!-- {#each} expression, 100 chars -->
<pre>{#each fn(aaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbb) as item}{item}{/each}</pre>

<!-- 101 chars - still inline -->
<pre>{#each fn(aaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbb) as item}{item}{/each}</pre>

<!-- {#each} key, 100 chars -->
<pre>{#each items as item (fn(aaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbb))}{item}{/each}</pre>

<!-- 101 chars - still inline -->
<pre>{#each items as item (fn(aaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbb))}{item}{/each}</pre>
