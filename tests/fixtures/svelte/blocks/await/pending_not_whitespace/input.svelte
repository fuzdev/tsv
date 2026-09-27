<!-- a pending branch of anything but ASCII whitespace is content: the full form stays -->
{#await promise}<!-- c -->{:then value}{value}{/await}

{#await promise}&nbsp;{:then value}{value}{/await}

{#await promise}&#32;{:then value}{value}{/await}

{#await promise} {:then value}{value}{/await}

<!-- a comment-only catch body keeps the section as written -->
{#await promise}text{:catch}<!-- c -->{/await}
