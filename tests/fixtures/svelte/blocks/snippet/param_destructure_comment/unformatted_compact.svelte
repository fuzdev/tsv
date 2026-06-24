<!-- object default: comment after the `=` -->
{#snippet fn1({a=/* c */1})}<span>x</span>{/snippet}

<!-- object default: comment before the `=` (stays on the binding) -->
{#snippet fn2({b /* c */=1})}<span>x</span>{/snippet}

<!-- object: leading (after {), between properties, and trailing (before }) -->
{#snippet fn3({/* c */ c,/* c */ d /* c */})}<span>x</span>{/snippet}

<!-- object rename key:value: comment before and after the colon -->
{#snippet fn4({e /* c */:f})}<span>x</span>{/snippet}

{#snippet fn5({g:/* c */ h})}<span>x</span>{/snippet}

<!-- object computed key: comment inside the brackets -->
{#snippet fn6({[/* c */ k]:v})}<span>x</span>{/snippet}

<!-- object rest: comment between ... and the binding -->
{#snippet fn7({.../* c */ rest})}<span>x</span>{/snippet}

<!-- array: leading (after [), between elements, trailing (before ]) -->
{#snippet fn8([/* c */ m,n /* c */])}<span>x</span>{/snippet}

<!-- array rest -->
{#snippet fn9([.../* c */ tail])}<span>x</span>{/snippet}

<!-- nested: comment in an inner object default -->
{#snippet fn10([{p=/* c */1}])}<span>x</span>{/snippet}

<!-- default VALUE is an object/array expression: interior comment preserved, kept inline -->
{#snippet fn11({r={s:/* c */1}})}<span>x</span>{/snippet}

{#snippet fn12({t=[/* c */1]})}<span>x</span>{/snippet}

<!-- plain param default: comment after the `=` (no destructuring) -->
{#snippet fn13(a=/* c */1)}<span>x</span>{/snippet}
