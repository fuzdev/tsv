<!--
	The break in front of a format-ignored node is the AUTHORED gap, emitted once. An
	authored blank line in that gap is a Tier-2 signal like any other and survives; the
	directive freezes the node beside it, not the run that separates them.
-->

<!-- a blank between the directive and its frozen node survives, at the root -->
<!-- prettier-ignore -->  
  
<span    data-attr="value"   >text1</span>

<!-- ... inside an element, with a sibling before it -->
<div>
	text1

	<!-- prettier-ignore -->  
  
	<span    data-attr="value"   >text2</span>
</div>

<!-- ... and in a block body, which the same blank also forces open -->
{#if cond}
	<!-- prettier-ignore -->  
  
	<span    data-attr="value"   >text1</span>
{/if}
{#each items as item}
	<!-- prettier-ignore -->  
  
	<span    data-attr="value"   >text1</span>
{/each}

<!-- every frozen node kind answers alike: a component, a tag, a block -->
<div>
	<!-- prettier-ignore -->  
  
	<Comp    prop1="value1"     prop2="value2"    />
</div>
<div>
	<!-- prettier-ignore -->  
  
	{   expr   }
</div>
<div>
	<!-- prettier-ignore -->  
  
	{#if   cond}
<span>text1</span>
{/if}
</div>

<!-- control: a single newline is one break, the gap the author actually wrote -->
<div>
	<!-- prettier-ignore -->
	<span    data-attr="value"   >text1</span>
</div>

<!-- control: no blank at all, so nothing is preserved and nothing is invented -->
<div>
	text1
	<!-- prettier-ignore -->
	<span    data-attr="value"   >text2</span>
</div>
