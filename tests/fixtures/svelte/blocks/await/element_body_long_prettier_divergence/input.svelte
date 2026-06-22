<!-- then shorthand -->
<Container>
	{#await promise then value}
		<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5} />
	{/await}
</Container>

<!-- catch shorthand -->
<Container>
	{#await promise catch error}
		<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5} />
	{/await}
</Container>
