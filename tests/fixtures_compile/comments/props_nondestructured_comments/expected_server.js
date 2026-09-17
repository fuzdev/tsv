import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		// all props
		let { $$slots, $$events, ...props } = $$props;
		function describe() {
			// inner note
			return props.label;
		}
		$$renderer.push(`<p>${$.escape(describe())}</p>`);
	});
}
