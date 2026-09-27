<!--
	A `prettier-ignore` anywhere inside `<pre>` freezes the next node: inside an element, a
	component or a block section, at any depth, as it does at the top of the `<pre>`.
-->

<!-- inside an inline element, and two elements deep -->
<pre>a<b>b<!-- prettier-ignore --><i   a = "1" >text1</i><i  a = "1" >text1</i>c</b>d</pre>

<pre><code>a<b><!-- prettier-ignore --><i   a = "1" >text2</i><i  a = "1" >text2</i></b></code></pre>

<!-- inside a component -->
<pre>a<Comp>b<!-- prettier-ignore --><i   a = "1" >text3</i><i  a = "1" >text3</i>c</Comp>d</pre>

<!-- inside each section of an `{#if}` -->
<pre>{#if  cond}<!-- prettier-ignore --><i   a = "1" >text4</i><i  a = "1" >text4</i>{/if}</pre>

<pre>{#if  cond}a{:else if  cond2}<!-- prettier-ignore -->{  expr  }{ expr }{/if}</pre>

<pre>{#if  cond}a{:else}<!-- prettier-ignore --><Comp   a = "1"  /><Comp  a = "1"  />{/if}</pre>

<!-- inside an `{#each}` and its `{:else}` -->
<pre>{#each  arr  as  i}<!-- prettier-ignore --><i   a = "1" >text5</i><i  a = "1" >text5</i>{/each}</pre>

<pre>{#each  arr  as  i}a{:else}<!-- prettier-ignore -->{  expr  }{ expr }{/each}</pre>

<!-- inside each section of an `{#await}` -->
<pre>{#await  p}<!-- prettier-ignore -->{  expr  }{ expr }{:then v}a{/await}</pre>

<pre>{#await  p}a{:then v}<!-- prettier-ignore -->{  v  }{ v }{:catch e}a{/await}</pre>

<pre>{#await  p}a{:then v}a{:catch e}<!-- prettier-ignore -->{  e  }{ e }{/await}</pre>

<!-- inside a `{#key}` and a `{#snippet}` -->
<pre>{#key  k}<!-- prettier-ignore --><i   a = "1" >text6</i><i  a = "1" >text6</i>{/key}</pre>

<pre>{#snippet s()}<!-- prettier-ignore --><i   a = "1" >text7</i><i  a = "1" >text7</i>{/snippet}</pre>

<!--
	a frozen node holding a line break opens the tags of the element around it, as the same node
	unfrozen does: its line break is the `<pre>`'s content
-->
<pre>a<i
		><!-- prettier-ignore --><b   a = "1" >text8
text9</b></i
	>c</pre>

<pre>a<svelte:element this="i"
		><!-- prettier-ignore --><b   a = "1" >text8
text9</b></svelte:element
	>c</pre>
