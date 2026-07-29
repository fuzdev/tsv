<!--
	A whitespace-only separator standing between two non-text siblings makes the content a fill,
	and its run renders as one whitespace however it is spelled — so the separator's SPELLING
	carries no signal. A space and a tab are the same document and must reach the same form: the
	fully inline one, with the render-free content boundaries trimmed. Covers the three non-text
	sibling kinds — an expression-tag pair, an element pair and a component pair; beside a TAG the
	compiler keeps the run VERBATIM and the browser does the collapsing (see the README, which is
	where that distinction is argued). The spelling that is NOT interchangeable is a NEWLINE: a run
	of pure siblings with no prose keeps its authored lines, since without a fill to reflow into
	those lines are the only structure the author has (pinned by `inline_multiline_nontext`).
-->
<div>
	<span>
		{a}	{b}
	</span>
</div>

<div>
	<span>
		<code>a</code>	<code>b</code>
	</span>
</div>

<div>
	<span>
		<Comp prop="value" />	<Comp prop="value" />
	</span>
</div>
