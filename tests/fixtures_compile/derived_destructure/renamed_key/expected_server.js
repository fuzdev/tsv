import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { $$slots, $$events, ...o } = $$props;
	let x = $.derived(() => o.a),
		b = $.derived(() => o.b);
	$$renderer.push(`<!---->${$.escape(x())}${$.escape(b())}`);
}
