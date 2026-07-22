import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { $$slots, $$events, ...getObj } = $$props;
		let $$d = $.derived(getObj),
			a = $.derived(() => $$d().a);
		$$renderer.push(`<!---->${$.escape(a())}`);
	});
}
