import * as $ from 'svelte/internal/server';
function fixed($$renderer) {
	$$renderer.push(`<p>static</p>`);
}
export default function Input($$renderer) {
	// unused by the snippet
	let x = 1;
	fixed($$renderer);
	$$renderer.push(`<!----> <span>1</span>`);
}
