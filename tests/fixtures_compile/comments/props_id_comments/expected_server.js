import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	const // a stable id
		id = $.props_id($$renderer);
	function label() {
		// inner note
		return `label-${id}`;
	}
	$$renderer.push(`<p${$.attr('id', id)}>${$.escape(label())}</p>`);
}
