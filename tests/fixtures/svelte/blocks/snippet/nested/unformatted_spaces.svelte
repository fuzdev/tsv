{#snippet  outer(  )  }
	{#snippet  inner(  )  }
		<span  >nested</span>
	{/snippet }
	{@render  inner(  )  }
{/snippet }

{#snippet  recursive(  n  )  }
	{#if  n  >  0  }
		<span  >{  n  }</span>
		{@render  recursive(  n  -  1  )  }
	{:else  }
		<span  >done</span>
	{/if }
{/snippet }

{@render  outer(  )  }
{@render  recursive(  3  )  }
