<!-- an honored directive keeps its own line in a `{…}` value gap, so the value takes the
	broken block form -->
<button
	on:click={
		// prettier-ignore
		()  =>  fn()
	}
>
	text
</button>

<div
	class:active={
		// prettier-ignore
		a  &&  b
	}
	style:color={
		// prettier-ignore
		c  ||  d
	}
></div>

<p>
	{
		// prettier-ignore
		a  +  b
	}
</p>
