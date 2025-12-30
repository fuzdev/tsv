<div>
	<!-- Long arrow body: prettier doesn't wrap, we do -->
	{#await a  .  filter(
		(  item  )   =>
			item.a   &&   item.b   &&   item.c   &&   item.d   &&   item.e   &&   item.f   &&   item.g   &&   item.h   &&   item.i  ,
	)   then   item}{item}{/await}

	<!-- Long function args: prettier doesn't wrap, we do (109 chars) -->
	{#await  getData(
		aaaaaaaaaaaaaaaaaaa ,
		bbbbbbbbbbbbbbbbbbb ,
		ccccccccccccccccccc ,
	)  then  result}{result}{/await}
</div>
