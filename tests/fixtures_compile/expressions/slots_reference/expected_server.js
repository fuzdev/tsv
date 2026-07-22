import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	const $$slots = $.sanitize_slots($$props);
	$$renderer.push(`<p>${$.escape($$slots)}</p>`);
}
