<!--
	The break in front of a format-ignored node is the AUTHORED gap, printed once and never
	invented. A directive GLUED to the node it freezes stays glued: there is no whitespace at
	that boundary, so breaking it would inject a rendered space the source does not have — the
	same rule that already keeps the directive welded to whatever precedes it.
-->

<!-- glued to an element, in a body a block sibling holds open -->
{#if cond}
	<div>block1</div>
	text1<!-- prettier-ignore --><span    data-attr="value"   >text2</span>
{/if}

<!-- every frozen kind answers alike: a component, a tag -->
{#if cond}
	<div>block1</div>
	text1<!-- prettier-ignore --><Comp    prop1="value1"     prop2="value2"    />
{/if}
{#if cond}
	<div>block1</div>
	text1<!-- prettier-ignore -->{   expr   }
{/if}

<!-- ... and inside an element rather than a block body -->
<div>
	<p>block1</p>
	text1<!-- prettier-ignore --><span    data-attr="value"   >text2</span>
</div>

<!-- control: an authored newline IS a break, and one break is what it gets -->
{#if cond}
	<div>block1</div>
	<!-- prettier-ignore -->
	<span    data-attr="value"   >text2</span>
{/if}

<!-- control: an authored blank survives as a blank -->
{#if cond}
	<div>block1</div>

	<!-- prettier-ignore -->

	<span    data-attr="value"   >text2</span>
{/if}

<!-- the same rule in a HUGGED body: the glue holds, and an authored gap survives as the one
	space it renders as, whatever the author spelled it -->
{#if cond}text1<!-- prettier-ignore --><span    data-attr="value"   >text2</span>{/if}
{#if cond}text1<!-- prettier-ignore --> <span    data-attr="value"   >text2</span>{/if}
{#if cond}text1<!-- prettier-ignore --> <Comp    prop1="value1"     prop2="value2"    />{/if}
{#if cond}text1<!-- prettier-ignore --> {   expr   }{/if}
