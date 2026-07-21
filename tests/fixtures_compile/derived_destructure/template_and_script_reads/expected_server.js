import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { $$slots, $$events, ...o } = $$props;
	let a = $.derived(() => o.a),
		b = $.derived(() => o.b);
	const s = a() + b();
	$$renderer.push(`<!---->${$.escape(a())}${$.escape(b())}${$.escape(s)}`);
}
