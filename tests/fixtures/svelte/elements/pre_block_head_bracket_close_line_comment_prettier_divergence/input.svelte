<!-- A `//` in a computed member's index→`]` gap inside a whitespace-sensitive element: the
	comment ends its own line, so `]` drops below it — inside the braced head, where no
	character is content. -->
<pre>{#if obj[key // c
	]}a{/if}</pre>

<pre>{#each items as item (obj[key // c
	])}{item}{/each}</pre>
