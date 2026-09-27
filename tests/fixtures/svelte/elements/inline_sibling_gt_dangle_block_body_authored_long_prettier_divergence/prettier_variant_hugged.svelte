<!-- a {:then} section holding an element that holds a block -->
<b
	>{#await promise}text{:then value}<i
			>{#if cond}{value}{/if}</i
		>{/await}</b
>{#if cond}<div>block1</div>
	<div>block2</div>{/if}

<!-- a {#snippet} glued on both sides, its body a multi-line comment, inside a block parent -->
<div>
	<b>text1{#snippet fn()}<!-- comment1
comment2 -->{/snippet}text2</b>{#if cond}<div>block1</div>
		<div>block2</div>{/if}
</div>

<!-- a component, its section holding a multi-line attribute value, before a glued element -->
<Comp
	>{#await promise then value}<i
			title="value1
value2">{value}</i
		>{/await}</Comp
><i
	title="value1
value2">text</i
>

<!-- as the second element of a glued pair, after an element, inside an inline parent -->
<span
	><i>text</i><b
		title="value1
value2"
		>{#await promise then value}<i
				>{#if cond}{value}{/if}</i
			>{/await}</b
	>text</span
>

<!-- a section too wide for its line: the {#await} fits on its own line at 100 -->
<b
	>{#await promise then value}text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aa{/await}</b
>{#if cond}<div>block1</div>
	<div>block2</div>{/if}

<!-- a section too wide for its line: the {#await} breaks at 101 -->
<b
	>{#await promise then value}text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaa{/await}</b
>{#if cond}<div>block1</div>
	<div>block2</div>{/if}

<!-- the {#await} sections authored on their own lines -->
<b>
	{#await promise}
		text
	{/await}
</b>{#if cond}<div>block1</div>
	<div>block2</div>{/if}

<!-- a {:catch} section authored on its own lines -->
<b
	>{#await promise}text{:catch error}
		text
	{/await}</b
>{#if cond}<div>block1</div>
	<div>block2</div>{/if}

<!-- a {#snippet} glued on both sides, its body authored on its own lines -->
<b
	>text1{#snippet fn()}
		text
	{/snippet}text2</b
>{#if cond}<div>block1</div>
	<div>block2</div>{/if}

<!-- control: a section holding one block element -->
<b>{#await promise then value}<div>{value}</div>{/await}</b>{#if cond}<div>block1</div>
	<div>block2</div>{/if}

<!-- control: the same content with no block around it -->
<b
	><i
		>{#if cond}{value}{/if}</i
	></b
>{#if cond}<div>block1</div>
	<div>block2</div>{/if}
