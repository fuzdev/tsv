<!-- An inline element sibling immediately before a block: when the block expands, tsv dangles the element's closing > onto its own line so the block head starts fresh (</span⏎>{#if…}); when the block stays inline, the > keeps hugging. The dangle injects no whitespace between </span> and {#if}, so it is render-safe. Prettier keeps the block inline either way. -->

<!-- {#if} expands (body overflows) → the </span> closing > dangles -->
<div>
	<span>text</span>{#if cond}
		<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5} />
	{/if}
</div>

<!-- {#each} dangles the same way -->
<div>
	<span>text</span>{#each items as item}
		<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5} />
	{/each}
</div>

<!-- {#key} too — the rule is uniform across {#if}/{#each}/{#key} -->
<div>
	<span>text</span>{#key keyValueExpression}
		<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5} />
	{/key}
</div>

<!-- {#await} dangles too — completing all 5 block heads -->
<div>
	<span>text</span>{#await promise then value}
		<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5} />
	{/await}
</div>

<!-- {#snippet} dangles too -->
<div>
	<span>text</span>{#snippet foo()}
		<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5} />
	{/snippet}
</div>

<!-- control: short block stays inline → the > keeps hugging, no dangle -->
<div>
	<span>text</span>{#if cond}text{/if}
</div>
