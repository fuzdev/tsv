<!-- A breakable component {#if} body at the 100/101 boundary: at 100 the construct stays inline (both formatters); at 101 tsv drops the body to its own line while prettier hugs the } and breaks the component internally (prettier_variant_hug). -->
<!-- inline form == 100: stays inline -->
{#if cond}<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5ab} />{/if}

<!-- inline form == 101: body drops to its own line -->
{#if cond}
	<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5abc} />
{/if}
