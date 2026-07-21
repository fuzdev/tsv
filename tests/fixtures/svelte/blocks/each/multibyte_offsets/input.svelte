<!--
	Multibyte before the context on the same line: the simple identifier's injected
	loc character/column and the destructure pattern's +1 read-pattern columns must
	translate from byte to UTF-16 offsets.
-->
{#each ['中文'] as item}
	<div>{item}</div>
{/each}

{#each ['😀'] as [a, b]}
	<div>{a} {b}</div>
{/each}
