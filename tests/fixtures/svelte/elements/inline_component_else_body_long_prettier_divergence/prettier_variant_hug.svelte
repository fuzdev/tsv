<!-- 100 chars: component with expression content fits on one line -->
<p>
	{#if a}text1 text2 text3 text4 text5 text6 text7 text8 text9 text10{:else}<Comp>{text}</Comp>{/if}
</p>

<!-- 101 chars: construct overflows → the whole if drops (prettier hugs the } and breaks </Comp>) -->
<p>
	{#if a}text1 text2 text3 text4 text5 text6 text7 text8 text9 text10a{:else}<Comp>{text}</Comp
		>{/if}
</p>

<!-- 106 chars: same full drop at higher width -->
<p>
	{#if a}text1 text2 text3 text4 text5 text6 text7 text8 text9 text10abcdef{:else}<Comp>{text}</Comp
		>{/if}
</p>

<!-- HTML inline element with expression content: 100 chars fits -->
<p>
	{#if a}text1 text2 text3 text4 text5 text6 text7 text8 text9 te{:else}<strong>{text}</strong>{/if}
</p>

<!-- HTML inline element at 101 chars: same full drop -->
<p>
	{#if a}text1 text2 text3 text4 text5 text6 text7 text8 text9 tea{:else}<strong>{text}</strong
		>{/if}
</p>
