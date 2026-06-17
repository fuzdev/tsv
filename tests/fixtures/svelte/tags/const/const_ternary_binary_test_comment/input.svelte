{#if x}
	{@const y =
		aaaaa === bbbbb // comment that pushes the const test line well over the print width xxxxxxxxxxxx
			? 1
			: 2}
{/if}
