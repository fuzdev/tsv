<!-- an own-line directive in a prefixed tag's head gap freezes the whole value -->
{@html
	// prettier-ignore
	aaa  +  bbb
}

<!-- an own-line block comment behaves identically -->
{@html
	/* prettier-ignore */
	ccc  +  ddd
}

<!-- `{@render}` is the same head, one keyword out -->
{@render
	// prettier-ignore
	fn  (  eee  )
}

<!-- `{@attach}` likewise, in attribute position -->
<div
	{@attach
		// prettier-ignore
		fn  (  ggg  )
	}
></div>

<!-- an ordinary trailing comment coexists with the freeze: its own break drops the `}`,
	dedented out of the frozen content, so the head adds no second break and the `}` lands
	where it does with no trailing comment at all -->
{@html
	// prettier-ignore
	jjj  +  kkk // trailing
}

<!-- a sibling tag the freeze does not reach still normalizes -->
{@html hhh + iii}
