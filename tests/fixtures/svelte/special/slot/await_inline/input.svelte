<slot>{#await promise}text{/await}</slot>

<slot name="x">{#await promise}loading{:then value}done{/await}</slot>
