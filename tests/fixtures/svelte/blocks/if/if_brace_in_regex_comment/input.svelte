<!-- `}` inside a regex / comment must not close the {#if} early -->
{#if f(/[}]/)}a{/if}
{#if g(/* } */ x)}b{/if}
