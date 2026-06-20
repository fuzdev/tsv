<!-- A breakable component preceded by a NON-FIRST node (leading text, a comment, or a void element) inside one {#if} body still drops uniformly — these are the positions that broke the old breakable-hug path. tsv drops the whole body; prettier hugs the } and breaks the component internally (prettier_variant_hug). -->
<div>
	<!-- leading text before the breakable component -->
	{#if cond}leading text node here before it <Comp
			prop1={value1}
			prop2={value2}
			prop3={value3}
		/>{/if}

	<!-- leading comment before the breakable component -->
	{#if cond}<!-- c --><Comp
			prop1={value1}
			prop2={value2}
			prop3={value3}
			prop4={value4}
			prop5={value5}
		/>{/if}

	<!-- leading void element before the breakable component -->
	{#if cond}<br /><Comp
			prop1={value1}
			prop2={value2}
			prop3={value3}
			prop4={value4}
			prop5={value5}
		/>{/if}
</div>
