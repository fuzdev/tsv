import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { $$slots, $$events, ...o } = $$props;
	let a = $.derived(() => $.fallback(o.a, 9));
	$$renderer.push(`<!---->${$.escape(a())}`);
}
