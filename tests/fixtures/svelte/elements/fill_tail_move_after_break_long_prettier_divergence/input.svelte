<!--
	A text run whose first word no longer fits after a predecessor that carries a FORCED break:
	the whole run starts a fresh line and the boundary space is spent on that break, never
	re-emitted at the head of the continuation line. The boundary is the run's own fill `line`,
	measured from the predecessor's actual end column, so it hugs while the first word fits and
	moves the run whole when it does not — the same answer for an inline element and an
	expression tag, and in the non-terminal and terminal positions alike.
-->

<!-- 100: after a multiline element, the first word hugs the intact closing tag at print width -->
<p>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">
		<code>text1</code>
	</span> text2xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
	text3 <b>x</b>
</p>

<!-- 101: one wider — the run moves whole, and no space leads its fresh line -->
<p>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">
		<code>text1</code>
	</span>
	text2xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx text3
	<b>x</b>
</p>

<!-- 100: the same element boundary in TERMINAL position — the first word still hugs -->
<p>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">
		<code>text1</code>
	</span> text2xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
	text3
</p>

<!-- 101: terminal, one wider — the run moves whole -->
<p>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">
		<code>text1</code>
	</span>
	text2xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx text3
</p>

<!-- 100: an expression TAG whose expression forces the break answers the same — the word hugs -->
<p>
	{fn(() => {
		return expr;
	})} text2xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
	text3 <b>x</b>
</p>

<!-- 101: one wider — the run moves whole, unled by a space -->
<p>
	{fn(() => {
		return expr;
	})}
	text2xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
	text3 <b>x</b>
</p>

<!-- 101: the tag boundary in TERMINAL position, the control on the position axis -->
<p>
	{fn(() => {
		return expr;
	})}
	text2xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
	text3
</p>

<!-- 101: a `svelte:*` element reaches the same answer through its own printer -->
<p>
	<svelte:component this={Comp} prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">
		<code>text1</code>
	</svelte:component>
	text2xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx text3 <b>x</b>
</p>
