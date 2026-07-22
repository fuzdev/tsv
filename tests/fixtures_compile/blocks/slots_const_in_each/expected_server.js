import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	const $$slots = $.sanitize_slots($$props);
	$$renderer.push(`<!--[-->`);
	const each_array = $.ensure_array_like([1]);
	for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
		let n = each_array[$$index];
		const $$slots = n;
		$$renderer.push(`<p>${$.escape($$slots)}</p>`);
	}
	$$renderer.push(`<!--]-->`);
}
