import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { $$slots, $$events, ...o } = $$props;
	function f() {
		return 1;
	}
	let a = $.derived(() => $.fallback(o.a, f, true));
	$$renderer.push(`<!---->${$.escape(a())}`);
}
