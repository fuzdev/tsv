<!--
	The sibling-newline flow rule's prose gate does not read the container: a one-word run is a
	label and holds its authored newlines at the root, in an inline element, a block element, a
	list item, a table cell, and inside a control-flow block or a snippet body alike, exactly as
	the flowing side of the rule lays one run out identically across every container kind
	(`container_kind_newline_flow`). Prettier holds every label shape too; the flowing control is
	the divergence.
-->

<!-- at the root -->
<Comp1 />
text1
<Comp2 />

<!-- in an inline element: an icon and its caption -->
<button>
	<Icon />
	text1
</button>

<!-- in a list item and a table cell -->
<ul>
	<li>
		<Icon />
		text1
	</li>
</ul>
<table>
	<tbody>
		<tr>
			<td>
				<Icon />
				text1
			</td>
		</tr>
	</tbody>
</table>

<!-- inside a control-flow block, and inside a snippet body -->
{#if cond}
	<Icon />
	text1
{/if}
{#snippet row()}
	<Icon />
	text1
{/snippet}

<!-- control: a two-word run in the same inline container is prose, and flows -->
<button>
	<Icon /> text1 text2
</button>
