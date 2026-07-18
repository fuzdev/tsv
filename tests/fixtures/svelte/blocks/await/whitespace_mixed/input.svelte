<!-- pending body spaced, catch body glued -->
{#await promise} a {:catch error}b{/await}

<!-- pending body glued, then body spaced -->
{#await promise}a{:then value} b {/await}

<!-- pending spaced, then body glued both sides, catch body spaced -->
{#await promise} a {:then value}b{:catch error} c {/await}
