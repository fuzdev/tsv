{#snippet fn1(
	v = x +
		y * z, // c1
	w
)}
	<p>{v}</p>
{/snippet}

{#snippet fn2(
	v = x -
		(y - z) // c2
)}
	<p>{v}</p>
{/snippet}

{#snippet fn3({
	v = a instanceof
		b // c3
})}
	<p>{v}</p>
{/snippet}

{#snippet fn4(
	v = a +
		(b, c), // c4
	w
)}
	<p>{v}</p>
{/snippet}
