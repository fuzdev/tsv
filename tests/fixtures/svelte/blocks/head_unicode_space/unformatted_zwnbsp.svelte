<!--
	Every gap inside a block head takes any JS `\s` character, so U+FEFF separates
	the tokens at each of them (see unformatted_zwnbsp). U+0085 NEL is not in JS
	`\s` and is rejected at every one (see the input_invalid_nel_* files).
-->
{#each items as item﻿}
	<p>text1</p>
{/each﻿}

{#each items as item,﻿i}
	<p>text2 {i}</p>
{/each}

{#await promise}
	<p>text3</p>
{:then﻿value}
	<p>text4 {value}</p>
{:catch﻿e}
	<p>text5 {e}</p>
{/await}

{#snippet﻿fn﻿()﻿}
	<p>text6</p>
{/snippet}

{@render fn()}
