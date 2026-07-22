import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	const $$slots = $.sanitize_slots($$props);
	function f($$slots) {
		return $$slots;
	}
	if ($$slots.a) {
		$$renderer.push('<!--[0-->');
		$$renderer.push(`<p>${$.escape(f(1))}</p>`);
	} else {
		$$renderer.push('<!--[-1-->');
	}
	$$renderer.push(`<!--]-->`);
}
