<!-- a non-ASCII identifier character ends an operand, so the `/` after it divides -->
<div>{é / 2}</div>
<div>{aé / 2}</div>
<div>{変数 / 2}</div>
<div>{éin / 2}</div>
<div data-attr={é / 2}></div>
{#if é / 2}text{/if}

<!-- an operator ends none, so the `/` after it opens a regex (the variant spaces it with U+00A0) -->
<div>{a && /}/.test(b)}</div>
