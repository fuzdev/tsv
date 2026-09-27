<!-- inside <pre> a whitespace-only section renders, so the block keeps it as written -->
<pre>{#await promise} {:then value}{value}{/await}</pre>

<pre>{#await promise}	{:then}text{/await}</pre>

<pre>{#await promise}  {:catch error}{error}{/await}</pre>

<pre>{#await promise} {:then value} {value} {:catch error}{error}{/await}</pre>

<pre>{#await promise}
{:then value}{value}{/await}</pre>

<!-- a whitespace-only {:then} is the text shown once the promise resolves -->
<pre>{#await promise then value} {/await}</pre>

<pre>{#await promise}text{:then} {:catch error}{error}{/await}</pre>

<!-- a catch section written first prints after the then section -->
<pre>{#await promise} {:catch error}{error}{:then value}{value}{/await}</pre>

<pre>{#await promise catch error}{error}{:then value} {/await}</pre>

<!-- an empty {:catch} keeps its place after the pending branch -->
<pre>{#await promise} {:catch error}{/await}</pre>

<!-- the whitespace-preserving state reaches every descendant fragment -->
<pre><b>{#await promise} {:then value}{value}{/await}</b></pre>

<pre>{#if cond}{#await promise} {:then value}{value}{/await}{/if}</pre>

<pre><Comp>{#await promise} {:then value}{value}{/await}</Comp></pre>

<pre><svelte:element this="b">{#await promise} {:then value}{value}{/await}</svelte:element></pre>
