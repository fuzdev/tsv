<!-- space-only section boundaries are render-free (compile-time trim), so they glue inline -->
{#await promise}text1{:then value}text2{:catch error}text3{/await}

{#await promise}text1{:then value}text2{/await}

{#await promise}text1{:catch error}text2{/await}

{#await promise then value}text1{/await}
