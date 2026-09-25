<!--
	An inline element or component holding a block element beside an {#await} is mixed content,
	as it is beside an {#if} or an expression tag: it lays out multiline, the block element and
	the {#await} each on a line of their own, in either order and at any width. A parent holding
	no block element keeps a short {#await} inline. Prettier keeps this form once written, but
	lays the compact authoring out with the parent's delimiters dangled.
-->

<!-- a block element too wide for its line beside an {#await} -->
<b>
	<div class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
		{y}
	</div>
	{#await p}x{/await}
</b>

<!-- the same at any width -->
<b>
	<div>{y}</div>
	{#await p}x{/await}
</b>

<!-- the {#await} before the block element -->
<b>
	{#await p}x{/await}
	<div class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
		{y}
	</div>
</b>

<!-- the same at any width -->
<b>
	{#await p}x{/await}
	<div>{y}</div>
</b>

<!-- a then clause -->
<b>
	<p class="cccccccccccccccccccccccccccccccccccccccc">{y}</p>
	{#await p then v}{v}{/await}
</b>

<!-- a catch branch -->
<b>
	<div>{y}</div>
	{#await p}x{:catch e}{e}{/await}
</b>

<!-- a space between them -->
<b>
	<p class="cccccccccccccccccccccccccccccccccccccccc">{y}</p>
	{#await p}x{/await}
</b>

<!-- a component parent -->
<Comp>
	<div class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
		{y}
	</div>
	{#await p}x{/await}
</Comp>

<!-- a block parent, the {#await} first -->
<div>
	{#await p}x{/await}
	<div class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
		{y}
	</div>
</div>

<!-- the same at any width -->
<div>
	{#await p}x{/await}
	<div>{y}</div>
</div>

<!-- in an inline parent -->
<span>
	<b>
		<div class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
			{y}
		</div>
		{#await p}x{/await}
	</b>
</span>

<!-- before a glued block: the parent keeps its > -->
<b>
	<div class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">
		{y}
	</div>
	{#await p}x{/await}
</b>{#if c}
	x
{/if}

<!-- nested -->
<b>
	<div class="cccccccccccccccccccccccccccccccccccccccc">
		<b>
			<div class="cccccccccccccccccccccccccccccccccccccccc">{y}</div>
			{#await p}x{/await}
		</b>
	</div>
	{#await p}x{/await}
</b>

<!-- control: an {#if} beside a block element already lays out this way -->
<b>
	<div>{y}</div>
	{#if c}x{/if}
</b>

<!-- control: with no block element, a short {#await} stays inline -->
<b><i>x</i>{#await p}x{/await}</b>
