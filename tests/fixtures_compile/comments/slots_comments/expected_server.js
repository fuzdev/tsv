import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	const $$slots = $.sanitize_slots($$props);
	// the label
	let { label } = $$props;
	function has_default() {
		// inner note
		return !!$$slots.default;
	}
	$$renderer.push(`<p>${$.escape(label)}${$.escape(has_default())}</p>`);
}
