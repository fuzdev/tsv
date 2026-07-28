<!-- an assignment value needs clarity parens, and those are the PRINTER's, so a frozen
	head keeps them outside the verbatim slice — the interior stays as authored -->
{@html
	// prettier-ignore
	(jjj  =  kkk)
}

<!-- a block head takes the same shape -->
{#if
	// prettier-ignore
	(lll  =  mmm)
}
	text1
{/if}

<!-- an unfrozen sibling still normalizes, parens and all -->
{@html (nnn = ooo)}
