<!-- A breakable component {#each} body at the 100/101 boundary: at 100 the construct stays inline (both formatters); at 101 tsv drops the body to its own line while prettier hugs the } and breaks the component internally (prettier_variant_hug). -->
<!-- inline form == 100: stays inline -->
{#each items as item}<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4abcd} />{/each}

<!-- inline form == 101: body drops to its own line -->
{#each items as item}
	<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4abcde} />
{/each}
