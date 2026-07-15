<!-- {#await} inside <pre>: whitespace is significant, so the block stays inline without dangling `}` or expanding its body -->
<pre>{#await promise}loading{:then value}{value}{:catch error}{error}{/await}</pre>

<!-- {#await} shorthand (then in the opening tag) -->
<pre>{#await promise then value}{value}{/await}</pre>

<!-- {#await} pending-only -->
<pre>{#await promise}loading{/await}</pre>

<!-- {#key} inside <pre> -->
<pre>{#key id}text{/key}</pre>

<!-- {#snippet} inside <pre> -->
<pre>{#snippet fn()}text{/snippet}</pre>
