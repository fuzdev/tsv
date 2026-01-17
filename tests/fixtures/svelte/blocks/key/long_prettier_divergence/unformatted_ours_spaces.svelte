<div>
	<!-- 100 chars - stays inline (at boundary, both agree) -->
	{#key  fn00000(  aaaaaaaaaaaaaaaaaaa ,  bbbbbbbbbbbbbbbbbbb ,  ccccccccccccccccccccccccccccccc  )}{x}{/key}

	<!-- 101 chars - args wrap (divergence: Prettier keeps inline) -->
	{#key  fn00000(
		aaaaaaaaaaaaaaaaaaa ,
		bbbbbbbbbbbbbbbbbbb ,
		cccccccccccccccccccccccccccccccc ,
	)}{x}{/key}

	<!-- Long filter with arrow body (divergence: call breaks open) -->
	{#key  a . filter(
		( item )  =>
			item . a  &&  item . b  &&  item . c  &&  item . d  &&  item . e  &&  item . f  &&  item . g  &&  item . h  &&  item . i ,
	)}{x}{/key}
</div>
