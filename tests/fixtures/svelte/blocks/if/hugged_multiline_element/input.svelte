<!-- Short condition stays inline even when element wraps -->
{#if a.b === 'value'}<Comp
		prop1={cond ? VALUE_A : VALUE_B}
		prop2={cond ? 'text1 text2' : 'text3 text4'}
	/>{/if}
