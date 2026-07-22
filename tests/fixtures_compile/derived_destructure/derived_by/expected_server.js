import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { $$slots, $$events, ...o } = $$props;
	let $$d = $.derived(() => o),
		a = $.derived(() => $$d().a);
	$$renderer.push(`<!---->${$.escape(a())}`);
}
