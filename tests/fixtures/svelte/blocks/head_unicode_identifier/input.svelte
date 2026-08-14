<!--
	A {#snippet} name and an {#each} index are JS identifiers, so they take the
	Unicode ID_Start / ID_Continue classes rather than Rust's alphanumeric one:
	U+2118 is ID_Start but not alphabetic, and U+200C ZWNJ is ID_Continue but not
	alphanumeric. U+00B2 is the reverse (see input_invalid_sup2_each_index).
-->
{#snippet ℘()}
	<p>text1</p>
{/snippet}

{#snippet fn‌()}
	<p>text2</p>
{/snippet}

{@render ℘()}
{@render fn‌()}

{#each items as item, ℮}
	<p>text3 {℮}</p>
{/each}

{#each items as item, i‌}
	<p>text4 {i‌}</p>
{/each}
