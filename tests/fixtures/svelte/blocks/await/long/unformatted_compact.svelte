<div>
<!-- 2-chain: 100 chars - stays inline (at boundary) -->
{#await a.filter((item) => item.x).filter((item) => item.yyyyyyyyyyyyyyy) then item}{item}{/await}

<!-- 2-chain: 101 chars - wraps (exceeds print_width) -->
{#await a.filter((item) => item.x).filter((item) => item.yyyyyyyyyyyyyyyy) then item}{item}{/await}

<!-- 3+ chain: always wraps regardless of width -->
{#await a.filter((item) => item.x).map((item) => item.y).filter((item) => item.z) then item}{item}{/await}

<!-- With {:then} block: 100 chars - stays inline -->
{#await a.filter((item) => item.x).filter((item) => item.yyyyyyyyyyyyyyyyyy)}x{:then i}{i}{/await}

<!-- With {:then} block: 101 chars - wraps -->
{#await a.filter((item) => item.x).filter((item) => item.yyyyyyyyyyyyyyyyyyy)}x{:then i}{i}{/await}
</div>
