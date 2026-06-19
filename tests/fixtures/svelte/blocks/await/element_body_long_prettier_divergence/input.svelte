<!-- then shorthand -->
<Container
	>{#await promise then value}
		<Comp prop1={some_longer_variable_name} prop2={another_variable_name_here} />
	{/await}</Container
>

<!-- catch shorthand -->
<Container
	>{#await promise catch error}
		<Comp prop1={some_longer_variable_name} prop2={another_variable_name_here} />
	{/await}</Container
>
