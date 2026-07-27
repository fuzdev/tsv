{#if cond}
	<!-- an own-line directive in a `{@const}`'s `=`→value gap freezes the whole value -->
	{@const aaa   =
		// prettier-ignore
		bbb  +  ccc}

	<!-- an own-line block comment behaves identically -->
	{@const ddd   =
		/* prettier-ignore */
		eee  +  fff}

	<!-- a destructuring binding freezes the same way -->
	{@const {   ggg   }   =
		// prettier-ignore
		hhh  ||  iii}

	<!-- a self-expanding value hangs under the `=` like any other, so the directive keeps
	its own line -->
	{@const jjj   =
		// prettier-ignore
		{
				kkk:   1
		}}
{/if}
