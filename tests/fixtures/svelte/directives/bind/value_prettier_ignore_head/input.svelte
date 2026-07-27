<!-- an own-line directive in the `{`→value gap freezes the whole bind value -->
<input
	bind:value={
		// prettier-ignore
		a  .  b
	}
/>

<!-- a multi-line frozen value keeps its verbatim layout -->
<input
	bind:value={
		// prettier-ignore
		fn(
			a,
				b
		)
	}
/>

<!-- a value the freeze does not reach still normalizes -->
<input
	bind:value={
		// prettier-ignore
		a  .  b
	}
	bind:checked={c.d}
/>
