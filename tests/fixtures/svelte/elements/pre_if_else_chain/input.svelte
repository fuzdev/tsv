<!-- {:else if} chain inside <pre>: whitespace is significant, so the flattened chain stays inline without expanding -->
<pre>{#if a}x{:else if b}y{:else}z{/if}</pre>

<!-- three-level chain exercises the recursive {:else if} flattening -->
<pre>{#if a}w{:else if b}x{:else if c}y{:else}z{/if}</pre>

<!-- chain without a final {:else} -->
<pre>{#if a}x{:else if b}y{/if}</pre>
