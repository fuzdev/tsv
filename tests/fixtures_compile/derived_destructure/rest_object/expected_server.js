import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { $$slots, $$events, ...o } = $$props;
	let a = $.derived(() => o.a),
		r = $.derived(() => $.exclude_from_object(o, ['a']));
	$$renderer.push(`<!---->${$.escape(a())}${$.escape(r())}`);
}
