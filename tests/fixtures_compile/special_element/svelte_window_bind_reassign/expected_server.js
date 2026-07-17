import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let y = 0;
	$$renderer.push(`<!---->${$.escape(y)}`);
}
