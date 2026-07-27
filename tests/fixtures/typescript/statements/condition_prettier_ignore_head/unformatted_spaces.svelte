<script lang="ts">
	// an own-line directive in the `(`→condition gap freezes the whole condition
	if  (
		// prettier-ignore
		aaa  &&  bbb
	)  {
		fn( );
	}

	// `while` shares the condition layout, so it shares the freeze
	while  (
		// prettier-ignore
		aaa  &&  bbb
	)  {
		fn( );
	}

	// `do…while` keeps its own comment placement and freezes the same way
	do  {
		fn( );
	}  while  (
		// prettier-ignore
		aaa  +  bbb
	)  ;

	// a `switch` discriminant is the same head
	switch  (
		// prettier-ignore
		aaa  +  bbb
	)  {
		case  1:
			break;
	}

	// a `catch` parameter is the same head; the binding pattern freezes whole
	try  {
		fn( );
	}  catch  (
		// prettier-ignore
		{ aaa , bbb }
	)  {
		fn( );
	}

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	if  (
		/* prettier-ignore */
		aaa  &&  bbb
	)  {
		fn( );
	}

	// a multi-line frozen condition keeps its verbatim layout
	if  (
		// prettier-ignore
		call(
			a,
				b
		)
	)  {
		fn( );
	}

	// an assignment condition's clarity parens are the printer's, so they stay OUTSIDE the
	// frozen slice
	if  (
		// prettier-ignore
		(  aaa  =  bbb  )
	)  {
		fn( );
	}
</script>
