<!-- A short condition with a breakable element body that exceeds printWidth: tsv drops the body to its own line (prettier hugs the } and breaks the element internally) -->
{#if a.b === 'value'}
	<Comp prop1={cond ? VALUE_A : VALUE_B} prop2={cond ? 'text1 text2' : 'text3 text4'} />
{/if}
