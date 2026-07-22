import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	const id = $.props_id($$renderer);
	$$renderer.push(`<!---->${$.escape(id)}`);
}
