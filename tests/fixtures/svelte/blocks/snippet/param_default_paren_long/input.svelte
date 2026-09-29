<!-- head is 100 chars with the kept pair - stays inline -->
{#snippet fn1(a = (x + y) * 2, b = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}
	<div>text</div>
{/snippet}

<!-- head is 101 chars with the kept pair - wraps -->
{#snippet fn2(
	a = (x + y) * 2,
	b = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
)}
	<div>text</div>
{/snippet}

<!-- last argument hugs -->
{#snippet fn3(
	a = f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, () => {
		return c;
	})
)}
	<div>text</div>
{/snippet}

<!-- member chain breaks after the kept pair -->
{#snippet fn4(
	a = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
		.cccccccccccccccccccccccccccccccc.dddddddd
)}
	<div>text</div>
{/snippet}
