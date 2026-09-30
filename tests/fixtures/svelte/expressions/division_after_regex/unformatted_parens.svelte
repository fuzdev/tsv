<!-- a regex literal ends an operand, so the `/` after it divides, with no flags as with them -->
<div>{x + (/a/) / 2}</div>
<div>{x + (/a/g) / 2}</div>
<div>{x + /a/ /* c */ / 2}</div>
<div data-attr={(/a/) / 2}></div>
{#if x + (/a/) / 2}text{/if}
{#if (/a/) / 2}text{/if}
