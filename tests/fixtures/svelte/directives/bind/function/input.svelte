<!-- Function bindings: bind:prop={getter, setter} (Svelte 5.9+) -->

<!-- Basic getter/setter -->
<input bind:value={() => val, (v) => (val = v)} />

<!-- With transformation in setter -->
<input bind:value={() => value, (v) => (value = v.toLowerCase())} />

<!-- Readonly bindings with null getter -->
<div bind:clientWidth={null, (w) => (width = w)}></div>
<div bind:clientHeight={null, (h) => (height = h)}></div>

<!-- Multiple readonly bindings on same element -->
<div bind:clientWidth={null, redraw} bind:clientHeight={null, redraw}>content</div>

<!-- Complex getter expression -->
<input bind:value={() => obj.nested.value, (v) => (obj.nested.value = v)} />
