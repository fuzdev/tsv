<!-- an author blank after a line comment leading a function-binding sequence survives, as at
	every other braced head -->
<input
	bind:value={
		// c
		(() => a, (v) => (a = v)
	}
/>
