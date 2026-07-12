<!-- <slot> whose only child is an expanding block ({#if}/{#each}/{#key}) with no boundary whitespace: forces hug mode (> and </slot dangle onto their own lines) -->
<slot
	>{#if cond}text{/if}</slot
>

<slot name="x"
	>{#each items as item}{item}{/each}</slot
>

<slot
	>{#key id}text{/key}</slot
>
