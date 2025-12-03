{#snippet fn1()}
<span>text</span>
{/snippet}

{#snippet fn2(a,b)}
<span>{a} {b}</span>
{/snippet}

{@render fn1()}
{@render fn2(expr1,expr2)}
