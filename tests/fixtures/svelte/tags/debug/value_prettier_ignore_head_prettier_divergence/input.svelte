<!-- an own-line directive in the `{@debug}` head gap freezes the identifier list -->
{@debug
	// prettier-ignore
	aaa ,  bbb
}

<!-- a sibling tag the freeze does not reach still normalizes -->
{@debug ccc, ddd}
