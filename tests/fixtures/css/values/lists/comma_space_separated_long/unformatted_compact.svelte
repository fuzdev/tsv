<style>
div {
/* 100 chars - stays inline (see 101 divergence fixture for 101+ behavior) */
box-shadow: var(--a) color-mix(in hsl, var(--b) var(--c, var(--d0000000000000000000000000)), transparent), var(--e) color-mix(in hsl, var(--f) var(--g, var(--h0000000000000000000000000)), transparent);

/* 102 chars - wraps (both formatters) */
box-shadow: var(--a) color-mix(in hsl, var(--b) var(--c, var(--d000000000000000000000000000)), transparent), var(--e) color-mix(in hsl, var(--f) var(--g, var(--h000000000000000000000000000)), transparent);
}
</style>
