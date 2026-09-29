<!-- a 100-column line: the comment stays on the value line, flat -->
<p>
	{aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa || b || c // c1
	}
</p>

<!-- a 101-column line: the chain breaks, the comment still ends its last line -->
<p>
	{aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
		b ||
		c // c2
	}
</p>

<!-- a hugged arrow at 100 columns: the call stays flat, the comment ends the line before `then` -->
{#await f(() => ayyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy || b || c) // c3
then v}
	<p>{v}</p>
{/await}

<!-- at 101 columns the call breaks, and the comment flushes inside it -->
{#await f(
	() => ayyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy || b || c // c4
) then v}
	<p>{v}</p>
{/await}

<!-- `{let}` at 100 columns: the comment ends the declaration, so the `;` carries it -->
{let r = f(() => ayyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy || b || c) // c5
;}

<!-- at 101 columns the call breaks and takes the comment, and the `;` is dropped with it -->
{let s = f(
	() => ayyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy || b || c // c6
)}

<!-- a `bind:` sequence at 100 columns: the comment ends the getter's line behind the comma -->
<input
	bind:value={
		() => ayyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy || b || c, // c7
		(v) => (a = v)
	}
/>

<!-- at 101 columns the getter breaks after its arrow, charged the comment it ends on -->
<input
	bind:value={
		() =>
			ayyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy || b || c, // c8
		(v) => (a = v)
	}
/>
