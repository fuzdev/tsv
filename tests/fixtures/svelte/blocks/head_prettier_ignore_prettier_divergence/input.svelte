<!-- an own-line directive in a block head's gap freezes the whole head expression -->
{#if
	// prettier-ignore
	aaa  +  bbb
}
	text1
{:else if
	// prettier-ignore
	ccc  +  ddd
}
	text2
{/if}

<!-- `{#each}` freezes the collection; the `as` clause stays parent-owned -->
{#each
	// prettier-ignore
	eee  .  fff
as item}
	text3
{/each}

<!-- `{#key}` and `{#await}` are the same head -->
{#key
	// prettier-ignore
	ggg  +  hhh
}
	text4
{/key}

{#await
	// prettier-ignore
	fn  (  iii  )
}
	text5
{/await}

<!-- an `{#each}` key is its own paren-delimited head -->
{#each jjj as kkk (
	// prettier-ignore
	lll  .  mmm
)}
	text6
{/each}

<!-- an ordinary trailing comment coexists with the freeze: its own hardline drops the
	`}`, so the head adds no second break -->
{#if
	// prettier-ignore
	nnn  +  ooo // trailing
}
	text7
{/if}
