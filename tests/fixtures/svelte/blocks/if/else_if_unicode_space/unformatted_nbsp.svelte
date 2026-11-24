<!--
	Svelte takes any JS `\s` character between `else` and `if`, so a vertical tab, a
	no-break space or a U+FEFF separates them (see the unformatted_* variants), and the
	head's expression still opens after the `if`: a `/` there starts a regex. U+0085 NEL
	is not in JS `\s` and does not separate them (see input_invalid_nel_else_if).
-->
{#if a}
	<p>text1</p>
{:else if /}/.test(b)}
	<p>text2</p>
{/if}
