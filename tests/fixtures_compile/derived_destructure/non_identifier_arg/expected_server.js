import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { $$slots, $$events, ...o } = $$props;
		let $$d = $.derived(() => o.x),
			a = $.derived(() => $$d().a);
		$$renderer.push(`<!---->${$.escape(a())}`);
	});
}
