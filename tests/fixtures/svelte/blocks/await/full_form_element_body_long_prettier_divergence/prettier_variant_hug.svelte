<!-- full {#await}/{:then}/{:catch} form where each section's breakable component body overflows: tsv drops every section body to its own line (uniform across sections), while prettier hugs each } and breaks the component internally (prettier_variant_hug) -->
<div>
	{#await promise}<Comp
			prop1={value1}
			prop2={value2}
			prop3={value3}
			prop4={value4}
			prop5={value5abc}
		/>{:then value}<Comp
			prop1={value1}
			prop2={value2}
			prop3={value3}
			prop4={value4}
			prop5={value5abc}
		/>{:catch error}<Comp
			prop1={value1}
			prop2={value2}
			prop3={value3}
			prop4={value4}
			prop5={value5abc}
		/>{/await}
</div>
