import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	const id = $.props_id($$renderer);
	const a = 1;
	$$renderer.push(`<p>1${$.escape(id)}</p>`);
}
