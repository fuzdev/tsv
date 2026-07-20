<svelte:head>
	<!-- direct child of head → TitleElement -->
	<title> text1 </title>

	<!-- inside a block → TitleElement (blocks are transparent) -->
	{#if cond}
		<title>  text2  </title>
	{/if}

	<!-- inside svelte:fragment → TitleElement (special elements are transparent) -->
	<svelte:fragment>
		<title> text3 </title>
	</svelte:fragment>

	<!-- inside a regular element → RegularElement (elements reset head context) -->
	<div>
		<title>text4</title>
	</div>
</svelte:head>
