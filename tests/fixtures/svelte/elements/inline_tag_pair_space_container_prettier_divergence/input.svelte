<!--
	The container holding a space-spelled tag pair is not an axis: the pair packs per width in
	every multiline container — the root, a block element, an inline element authored with air,
	a list item, a table cell, a component's content and a control-flow block's body — where
	prettier splits it in every one of them.
-->

<!-- the root, which is always a multiline container -->
{expr1} {expr2}

<!-- a block element, and inline elements authored with boundary air -->
<div>
	{expr1} {expr2}
</div>
<div>
	<span>
		{expr1} {expr2}
	</span>
</div>
<div>
	<button>
		{expr1} {expr2}
	</button>
</div>

<!-- a list item and a table cell -->
<ul>
	<li>
		{expr1} {expr2}
	</li>
</ul>
<table>
	<tr>
		<td>
			{expr1} {expr2}
		</td>
	</tr>
</table>

<!-- a component's content -->
<Comp>
	{expr1} {expr2}
</Comp>

<!-- control-flow block bodies -->
{#if cond}
	{expr1} {expr2}
{/if}
{#each items as item}
	{item.a} {item.b}
{/each}
{#snippet fn()}
	{expr1} {expr2}
{/snippet}
