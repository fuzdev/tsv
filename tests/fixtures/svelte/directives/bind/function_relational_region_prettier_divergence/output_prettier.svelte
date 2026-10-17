<script lang="ts">
	let lo = 0,
		hi = 1,
		a = 0,
		b = 1;
	function set(v: number) {}
</script>

<!-- A getter holding a `<` and a setter holding a `>` that may close the region: the setter's lone parameter written without a pair prints without one -->
<input bind:value={() => lo < hi, (v) => v > (lo + hi) / 2 && set(v)} />
<input
	bind:value={
		() => lowerBoundValue < upperBoundValue,
		(value) => value > (lowerBoundValue + upperBoundValue) / 2 && set(value)
	}
/>

<!-- The pair the author wrote behind `typeof` or a sign prints as written -->
<input bind:value={() => a < b, (v) => typeof v > (a, b)} />
<input bind:value={() => a < b, (v) => -1 > (a, b)} />

<!-- A comment beside the setter -->
<input bind:value={() => a < b, /* k */ (v) => v > (a, b)} />
<input
	bind:value={
		() => a < b, // k
		(v) => v > (a, b)
	}
/>

<!-- An operand's own pair around a comparison prints as written: the getter's around a value, and around an arrow that guards its body -->
<input bind:value={a < b, c > (d, e)} />
<input bind:value={() => lo < hi, (v) => v > (lo + hi) / 2 && set(v)} />
<input bind:value={/* k */ (a < b, c > (d, e))} />
<input
	bind:value={
		a < b,
		// prettier-ignore
		c  >  (d, e)
	}
/>
<input
	bind:value={
		a < b, // k
		c > (d, e)
	}
/>

<!-- The pair stays where no operand opens a region or none closes it -->
<input bind:value={() => lo, (v) => v > (lo + hi) / 2 && set(v)} />
<input bind:value={() => lo < hi, (v) => set(v)} />
<input bind:value={() => lo <= hi, (v) => v > (lo + hi) / 2 && set(v)} />
