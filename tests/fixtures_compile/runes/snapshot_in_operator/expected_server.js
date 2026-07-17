import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let state = { a: 1 };
	$$renderer.push(`<!---->${$.escape(2 in $.snapshot(state))}`);
}
