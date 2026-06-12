<div>
<!-- 100 chars - stays inline (at boundary, both agree) -->
{#await getData(aaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbb,cccccccccccccccccccc) then r}{r}{/await}

<!-- 101 chars - args wrap (divergence: Prettier keeps inline) -->
{#await getData(aaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbb,ccccccccccccccccccccc) then r}{r}{/await}

<!-- Long filter with arrow body (divergence: call breaks open) -->
{#await a.filter((item)=>item.a&&item.b&&item.c&&item.d&&item.e&&item.f&&item.g&&item.h&&item.i) then item}{item}{/await}
</div>
