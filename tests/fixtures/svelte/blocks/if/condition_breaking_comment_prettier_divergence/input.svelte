<!-- line comment breaks the head: continuation indents, } dangles (divergence) -->
{#if // c1
	cond
}
	<p>text</p>
{/if}

<!-- multiline block comment breaks the head; its interior stays verbatim, } dangles (divergence) -->
{#if /* c1
c2 */ cond
}
	<p>text</p>
{/if}

<!-- single-line block comment breaks nothing: head stays inline, both agree -->
{#if /* c1 */ cond}
	<p>text</p>
{/if}
