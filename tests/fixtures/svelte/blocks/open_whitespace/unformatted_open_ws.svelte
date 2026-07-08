{ #each items as item}
	{ @const a = item.name}
	<p>{a}</p>
{ /each}

{ #if cond}
	<p>text1</p>
{ :else}
	<p>text2</p>
{ /if}

{ @html expr}
