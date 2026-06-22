<!-- Basic inline snippet in element -->
<A>text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaa()}x{/snippet}</A>

<!-- With attribute (short) -->
<A href={x}>text{#snippet icon()}y{/snippet}</A>

<!-- Exactly 100 chars (fits on one line) -->
<A>text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()}x{/snippet}</A>

<!-- 101 chars (block-style: content drops to its own line) -->
<A>
	text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()}x{/snippet}
</A>

<!-- With attribute at 100 chars (fits) -->
<A href={x}>text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()}y{/snippet}</A>

<!-- With attribute at 101 chars (block-style) -->
<A href={x}>
	text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()}y{/snippet}
</A>

<!-- With attribute at 102 chars (block-style) -->
<A href={x}>
	ttext{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()}y{/snippet}
</A>

<!-- With params at 100 chars (fits) -->
<A>text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(a, b)}x{/snippet}</A>

<!-- With params at 101 chars (block-style) -->
<A>
	text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(a, b)}x{/snippet}
</A>
