import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		// bindables
		let {
			// the value
			value = void 0, /* none */ // trailing
			count = 0
		} = $$props;
		function reset() {
			// inner note
			value = undefined;
			count = 0;
		}
		$$renderer.push(`<button>${$.escape(value)}${$.escape(count)}</button>`);
		$.bind_props($$props, { value, count });
	});
}
