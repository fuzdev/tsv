<!-- a numeric literal may end in its `.` (the variant's `1.`), which ends an operand -->
<div>{1 / 2}</div>
<div data-attr={1 / 2}></div>
{#if 1 / 2}text{/if}

<!-- a spread's `...` ends no operand, so the `/` after it opens a regex -->
<div>{[.../}/.source]}</div>
