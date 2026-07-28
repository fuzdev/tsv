<!-- an own-line directive in a `{...}` spread's head gap freezes the whole value -->
<div
	{...
		// prettier-ignore
		aaa  .  bbb
	}
></div>

<!-- a sibling spread the freeze does not reach still normalizes -->
<div
	{...
		// prettier-ignore
		ccc  .  ddd
	}
	{...eee.fff}
></div>
