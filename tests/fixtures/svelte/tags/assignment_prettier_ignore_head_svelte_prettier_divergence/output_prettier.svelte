<!-- an assignment value needs clarity parens, and those are the PRINTER's, so a frozen
	head keeps them outside the verbatim slice — the interior stays as authored -->
{@html (jjj = kkk)}

<!-- a block head takes the same shape -->
{#if (lll = mmm)}
	text1
{/if}

<!-- an unprefixed `{…}` value takes the same shape — an attribute value, a spread head,
	an expression tag -->
<div prop={(ppp = qqq)} {...(rrr = sss)}></div>

<p>
	{(ttt = uuu)}
</p>

<!-- an unfrozen sibling still normalizes, parens and all -->
{@html (nnn = ooo)}
