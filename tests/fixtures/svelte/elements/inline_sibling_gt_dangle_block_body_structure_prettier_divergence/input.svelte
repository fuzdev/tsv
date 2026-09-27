<!-- a {:then} section holding two block elements -->
<b>
	{#await promise}
		<Comp />
	{:then value}
		<div>{value}</div>
		<div>block1</div>
	{/await}
</b>{#if cond}
	<div>block1</div>
	<div>block2</div>
{/if}

<!-- a pending section mixing text and a block element, in a component before an {#each} -->
<Comp>
	{#await promise}
		text
		<div>block1</div>
	{:then value}
		{value}
	{/await}
</Comp>{#each items as item}
	<div>{item}</div>
	<div>block1</div>
{/each}

<!-- a {#snippet} glued on both sides, its body two block elements, in a block parent -->
<div>
	<b>
		text1{#snippet fn()}
			<div>block1</div>
			<div>block2</div>
		{/snippet}text2
	</b>{#if cond}
		<div>block1</div>
		<div>block2</div>
	{/if}
</div>

<!-- before a glued element whose attribute value spans lines -->
<b>
	{#await promise then value}
		<div>{value}</div>
		<div>block1</div>
	{/await}
</b><i
	title="value1
value2"
>
	text
</i>

<!-- as the second element of a glued pair, after an element, inside an inline parent -->
<span>
	<i>text</i><b
		title="value1
value2"
	>
		{#await promise then value}
			<div>{value}</div>
			<div>block1</div>
		{/await}
	</b>text
</span>

<!-- a {:catch} section holding two block elements -->
<b>
	{#await promise}
		text
	{:catch error}
		<div>block1</div>
		<div>block2</div>
	{/await}
</b>{#if cond}
	<div>block1</div>
	<div>block2</div>
{/if}

<!-- a section holding a declaration on its own line -->
<b>
	{#await promise then value}
		{@const a = value}
		<i>{a}</i>
	{/await}
</b>{#if cond}
	<div>block1</div>
	<div>block2</div>
{/if}

<!-- a section holding an authored blank line between its children -->
<b>
	{#await promise then value}
		<i>inline1</i>

		<i>inline2</i>
	{/await}
</b>{#if cond}
	<div>block1</div>
	<div>block2</div>
{/if}

<!-- a section that is itself an {#await} whose section holds two block elements -->
<b>
	{#await promise then value}
		{#await promise2 then value2}
			<div>block1</div>
			<div>block2</div>
		{/await}
	{/await}
</b>{#if cond}
	<div>block1</div>
	<div>block2</div>
{/if}

<!-- a section that is itself an {#if} -->
<b>
	{#await promise then value}{#if cond}text{/if}{/await}
</b>{#if cond}
	<div>block1</div>
	<div>block2</div>
{/if}

<!-- the {#await} after text in the element -->
<b>
	text{#await promise then value}
		<div>{value}</div>
		<div>block1</div>
	{/await}
</b>{#if cond}
	<div>block1</div>
	<div>block2</div>
{/if}

<!-- inside an {#each} body -->
{#each items as item}
	<b>
		{#await promise then value}
			<div>{item}</div>
			<div>block1</div>
		{/await}
	</b>{#if cond}
		<div>block1</div>
		<div>block2</div>
	{/if}
{/each}

<!-- a section holding a block beside other content -->
<b>
	{#await promise then value}
		text{#await promise2}text{/await}
	{/await}
</b>{#if cond}
	<div>block1</div>
	<div>block2</div>
{/if}

<!-- a prettier-ignore'd {#await} whose section holds two block elements -->
<p>
	text1
	<b>
		<!-- prettier-ignore -->{#await promise}<div>block1</div><div>block2</div>{/await}
	</b> text2
</p>

<!-- control: the same content with no block around it -->
<b>
	<div>{value}</div>
	<div>block1</div>
</b>{#if cond}
	<div>block1</div>
	<div>block2</div>
{/if}
