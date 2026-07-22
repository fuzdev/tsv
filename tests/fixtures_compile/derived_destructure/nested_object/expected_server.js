import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { $$slots, $$events, ...o } = $$props;
	let c = $.derived(() => o.a.c);
	$$renderer.push(`<!---->${$.escape(c())}`);
}
