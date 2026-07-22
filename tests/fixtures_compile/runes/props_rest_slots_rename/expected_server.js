import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	const $$slots = $.sanitize_slots($$props);
	let { $$slots: $$slots_, $$events, ...rest } = $$props;
	if ($$slots.a) {
		$$renderer.push('<!--[0-->');
		$$renderer.push(`<p>${$.escape(rest)}</p>`);
	} else {
		$$renderer.push('<!--[-1-->');
	}
	$$renderer.push(`<!--]-->`);
}
