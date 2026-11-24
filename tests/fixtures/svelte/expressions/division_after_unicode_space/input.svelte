<!--
	Svelte takes any JS `\s` character between an operand and the `/` after it, so after a
	vertical tab, a no-break space or a U+FEFF (see the unformatted_* variants) the `/`
	still divides, in every host the brace scan serves. U+0085 NEL is not in JS `\s` (see
	input_invalid_nel_division).
-->
<p>{a / 2}</p>
<div x={a / 2}></div>
<div {@attach a / 2}></div>
{@html a / 2}
{#if a / 2}x{/if}
{#key a / 2}x{/key}
