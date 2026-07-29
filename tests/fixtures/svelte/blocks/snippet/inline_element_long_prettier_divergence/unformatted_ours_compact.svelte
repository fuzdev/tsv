<!-- Short: the snippet takes its own line, so the component goes block-style at any width -->
<A>text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaa()}x{/snippet}</A>

<!-- With attribute (short): same block-style form -->
<A href={x}>text{#snippet icon()}y{/snippet}</A>

<!-- Compact spelling exactly 100 chars: prettier keeps it on one line -->
<A>text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()}x{/snippet}</A>

<!-- Compact spelling at 101 chars: prettier dangles the closing > instead -->
<A>text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()}x{/snippet}</A>

<!-- With attribute at 100 chars: prettier keeps one line -->
<A href={x}>text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()}y{/snippet}</A>

<!-- With attribute at 101 chars: prettier dangles the closing > -->
<A href={x}>text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()}y{/snippet}</A>

<!-- With attribute at 102 chars: prettier dangles both tags -->
<A href={x}>ttext{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()}y{/snippet}</A>

<!-- With params at 100 chars: prettier keeps one line -->
<A>text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(a, b)}x{/snippet}</A>

<!-- With params at 101 chars: prettier dangles the closing > -->
<A>text{#snippet aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(a, b)}x{/snippet}</A>
